use std::{fmt::Debug, sync::Arc, time::Duration};
use discord_presence::models::{payload::Payload, Activity, ActivityAssets, ActivityType};
use apple_music::{MediaKind, Track};
use tokio::sync::Mutex;

use crate::{data_fetching::components::{Component, ComponentSolicitation}, util::fallback_to_default_and_log_absence};

use super::{Listened, StatusBackend};

const APPLICATION_ID: u64 = 1286481105410588672; // "Apple Music"

#[derive(thiserror::Error, Debug)]
pub enum ConnectError {
    // library will hang sometimes when discord is closed ; TODO: investigate & patch
    // this will also occur if a normal error occurs due to my workaround and it prevents it from reaching a "ready" state :shrug:
    #[error("timed out")]
    TimedOut
}

#[derive(thiserror::Error, Debug)]
pub enum UpdateError {
    #[error("{0}")]
    Lib(#[from] discord_presence::DiscordError),
    #[error("not connected out")]
    NotConnected
}

const CONNECTION_ATTEMPT_TIMEOUT: tokio::time::Duration = tokio::time::Duration::from_secs(3);
const TRY_AGAIN_DEBOUNCE: tokio::time::Duration = tokio::time::Duration::from_secs(7);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DiscordPresenceState {
    Ready,
    Disconnected,
}

macro_rules! get_client_or_early_return {
    ($self: ident) => {
        match *$self.state.try_lock().unwrap() {
            DiscordPresenceState::Disconnected => return,
            DiscordPresenceState::Ready => match $self.client.as_mut() {
                Some(client) => client,
                None => return
            }
        }
    };
}

pub struct DiscordPresence {
    client: Option<discord_presence::Client>,
    state: Arc<Mutex<DiscordPresenceState>>,
    state_channel: tokio::sync::broadcast::Sender<DiscordPresenceState>,
    state_update_task_handle: tokio::task::JoinHandle<()>,
    auto_reconnect_task_handle: Option<tokio::task::JoinHandle<()>>,
    has_content: bool,
    activity: Option<discord_presence::models::Activity>,
    position: f64,
    duration: f64,
}
impl Debug for DiscordPresence {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DiscordPresence").finish()
    }
}
impl Default for DiscordPresence {
    fn default() -> Self {
        Self::disconnected()
    }
}
impl DiscordPresence {
    #[tracing::instrument]
    pub async fn new() -> Self {
        let instance = Self::disconnected();
        let instance = instance.try_connect(CONNECTION_ATTEMPT_TIMEOUT).await;
        instance.unwrap_or_else(|_| {
            tracing::warn!("client creation timed out; assuming Discord isn't open");
            Self::disconnected()
        })
    }

    pub fn disconnected() -> Self {
        let (tx, mut rx) = tokio::sync::broadcast::channel(4);

        let state = Arc::new(Mutex::new(DiscordPresenceState::Disconnected));
        let update_v = state.clone();
        let state_update_hook = tokio::spawn(async move {
            loop {
                let state = rx.recv().await.expect("channel closed: all senders were dropped");
                *update_v.try_lock().unwrap() = state;
            }
        });

        Self {
            client: None,
            state,
            state_channel: tx,
            state_update_task_handle: state_update_hook,
            auto_reconnect_task_handle: None,
            has_content: false,
            activity: None,
            position: 0.,
            duration: 0.,
        }
    }

    pub async fn connect(mut self) -> Self {
        self.connect_in_place().await;
        self
    }

    /// not `tokio::select!` safe
    #[tracing::instrument]
    pub async fn connect_in_place(&mut self) {
        let client = discord_presence::Client::new(APPLICATION_ID);            
        if let Some(old_client) = self.client.replace(client) {
            // TODO: i assume this will set fire the tx and thus set state to disconnected, but i haven't tested
            if let Err(error) = old_client.shutdown() {
                tracing::warn!(?error, "could not shutdown client");
            }
        }
        let client = self.client.as_mut().unwrap();
                
        let mut rx_ready = self.state_channel.subscribe();         
        let tx_ready = self.state_channel.clone();
        let tx_disconnect = self.state_channel.clone();

        client.on_event(discord_presence::Event::Connected, move |_| {
            tx_ready.send(DiscordPresenceState::Ready).unwrap();
        }).persist();
        client.on_disconnected(move |_| {
            tx_disconnect.send(DiscordPresenceState::Disconnected).unwrap();
        }).persist();
        client.on_error(|err| {
            tracing::warn!("{:?}", &err);
        }).persist();
        client.start();


        loop {
            let state = rx_ready.recv().await.unwrap();
            if state == DiscordPresenceState::Ready { break }
        }
    }


    pub async fn try_connect(self, timeout: Duration) -> Result<Self, ConnectError> {
        let timeout = tokio::time::sleep(timeout);
        let handle = tokio::spawn(self.connect());
        let abortion_handle = handle.abort_handle();

        tokio::select! {
            outcome = handle => {
                Ok(outcome.expect("task did not finish successfully"))
            },
            _ = timeout => {
                abortion_handle.abort();
                Err(ConnectError::TimedOut)
            }
        }
    }

    /// not `tokio::select!` safe
    pub async fn try_connect_in_place(instance: Arc<Mutex<Self>>, timeout: Duration) -> Result<(), ConnectError> {
        tokio::time::timeout(
            timeout,
            instance.lock().await.connect_in_place()
        ).await
            .map(|_| ())
            .map_err(|_| ConnectError::TimedOut)
    }


    pub async fn enable_auto_reconnect(instance: Arc<Mutex<Self>>) {
        let mut rx = {
            let lock = instance.lock().await;

            if let Some(old_handle) = &lock.auto_reconnect_task_handle {
                old_handle.abort();
            };
            
            lock.state_channel.subscribe()
        };
        
        let task_instance: Arc<Mutex<DiscordPresence>> = instance.clone();

        let auto_reconnect_task_handle = tokio::spawn(async move {
            // If it's ready, wait for that to change, and then if it disconnects, reconnect. Repeat.
            // If it's disconnected, wait a bit before trying again. Repeat.
            loop {
                let state = { *task_instance.clone().lock().await.state.lock().await };
                let state = match state {
                    DiscordPresenceState::Ready => rx.recv().await.unwrap(),
                    DiscordPresenceState::Disconnected => {
                        tracing::debug!("disconnected; polling again in {:.2} seconds", TRY_AGAIN_DEBOUNCE.as_secs_f64());
                        tokio::time::sleep(TRY_AGAIN_DEBOUNCE).await;
                        DiscordPresenceState::Disconnected
                    }
                };
                match state {
                    DiscordPresenceState::Ready => continue,
                    DiscordPresenceState::Disconnected => {
                        if let Err(error) = Self::try_connect_in_place(
                            task_instance.clone(),
                            CONNECTION_ATTEMPT_TIMEOUT
                        ).await {
                            tracing::debug!(?error, "couldn't connect")
                        }
                    }
                }
            }
        });

        instance.lock().await.auto_reconnect_task_handle = Some(auto_reconnect_task_handle);
    }


    pub async fn client(&mut self) -> Option<&mut discord_presence::Client> {
        match *self.state.try_lock().unwrap() {
            DiscordPresenceState::Ready => self.client.as_mut(),
            DiscordPresenceState::Disconnected => None
        }
    }

    /// Returns whether the status was cleared.
    /// (If the status was already empty, it will return false.)
    #[tracing::instrument]
    pub async fn clear(&mut self) -> Result<bool, UpdateError> {
        let has_content = self.has_content;
        if let Some(client) = self.client().await {
            if has_content {
                client.clear_activity()?;
                self.has_content = false;
            }
            Ok(has_content)
        } else if !has_content {
            Ok(false) // is this a good idea
        } else {
            Err(UpdateError::NotConnected)
        }
    }

    async fn dispatch(&mut self) {
        let activity = if let Some(activity) = self.activity.clone() { activity } else {
            tracing::warn!("cannot dispatch without set activity");
            return
        };

        let client = if let Some(client) = self.client.as_mut() { client } else {
            tracing::warn!("cannot dispatch without client");
            return
        };

        match client.set_activity(|_| activity.timestamps(|activity| {
            let track_started_at = chrono::Utc::now().timestamp() as u64 - self.position as u64;
            activity
                .start(track_started_at)
                .end(track_started_at + self.duration as u64)
        })) {
            Ok(..) => {
                self.has_content = true;
            },
            Err(error) => {
                tracing::error!(?error, "activity dispatch failure");
            }
        }
    }
}
impl Drop for DiscordPresence {
    fn drop(&mut self) {
        self.state_update_task_handle.abort();
        if let Some(handle) = self.auto_reconnect_task_handle.as_ref() {
            handle.abort();
        }
    }
}
#[async_trait::async_trait]
impl StatusBackend for DiscordPresence {
    async fn get_additional_data_solicitation(&self) -> ComponentSolicitation {
        let mut solicitation: ComponentSolicitation = ComponentSolicitation::default();
        solicitation.list.insert(Component::ITunesData);
        solicitation.list.insert(Component::AlbumImage);
        solicitation.list.insert(Component::ArtistImage);
        solicitation
    }

    async fn record_as_listened(&self, _: Arc<Track>, _: Arc<apple_music::ApplicationData>) {
        // no-op
    }

    async fn check_eligibility(&self, _: Arc<Track>, _: Arc<tokio::sync::Mutex<super::Listened>>) -> bool {
        false
    }

    async fn update_progress(&mut self, _: Arc<Track>, progress: Arc<tokio::sync::Mutex<super::Listened>>) {
        self.position = progress.lock().await.current.as_ref().unwrap().get_expected_song_position();
        self.dispatch().await;
    }

    #[tracing::instrument(level = "debug")]
    async fn set_now_listening(&mut self, track: Arc<Track>, app: Arc<apple_music::ApplicationData>, additional_info: Arc<crate::data_fetching::AdditionalTrackData>) {
        self.position = app.player_position.unwrap_or_default();
        self.duration = track.duration;

        let mut activity = Activity::new()
            ._type(match track.media_kind {
                MediaKind::Song | 
                MediaKind::Unknown => ActivityType::Listening,
                MediaKind::MusicVideo => ActivityType::Watching,
            })
            .details(&track.name)
            .state(&track.artist)
            .assets(|_| ActivityAssets {
                large_text: Some(track.album.clone()),
                large_image: additional_info.images.track.clone(),
                small_image: additional_info.images.artist.clone(),
                small_text: Some(track.artist.clone())
            });


        if let Some(itunes) = &additional_info.itunes {
            activity = activity.append_buttons(|button| button
                .label("Listen on Apple Music")
                .url(itunes.apple_music_url.clone())
            )
        }

        self.activity = Some(activity);
        self.dispatch().await;
    }
}
