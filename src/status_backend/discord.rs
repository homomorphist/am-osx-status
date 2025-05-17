use std::{fmt::Debug, sync::{Arc, Weak}, time::Duration};
use discord_presence::models::{Activity, ActivityAssets, ActivityType};
use tokio::sync::Mutex;

use crate::data_fetching::components::{Component, ComponentSolicitation};

use super::error::DispatchError;

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

super::subscription::define_subscriber!(pub DiscordPresence, {
    client: Option<discord_presence::Client>,
    state: Arc<Mutex<DiscordPresenceState>>,
    state_channel: tokio::sync::broadcast::Sender<DiscordPresenceState>,
    state_update_task_handle: tokio::task::JoinHandle<()>,
    auto_reconnect_task_handle: Option<tokio::task::JoinHandle<()>>,
    has_content: bool,
    activity: Option<discord_presence::models::Activity>,
    position: Option<f32>,
    duration: Option<f32>,
});
impl Debug for DiscordPresence {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct(Self::NAME).finish()
    }
}
impl Default for DiscordPresence {
    fn default() -> Self {
        Self::disconnected()
    }
}
impl DiscordPresence {
    #[tracing::instrument(level = "debug")]
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
            position: None,
            duration: None,
        }
    }

    pub async fn connect(mut self) -> Self {
        self.connect_in_place().await;
        self
    }

    /// not `tokio::select!` safe
    #[tracing::instrument(skip(self), level = "debug")]
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
            tracing::error!(?err, "discord rpc error");
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


    pub async fn enable_auto_reconnect(instance: Weak<Mutex<Self>>) {
        let mut rx = if let Some(instance) = instance.upgrade() {
            let lock = instance.lock().await;

            if let Some(old_handle) = &lock.auto_reconnect_task_handle {
                old_handle.abort();
            };
            
            lock.state_channel.subscribe()
        } else { return };

        let sent = instance.clone();

        let auto_reconnect_task_handle = tokio::spawn(async move {
            // If it's ready, wait for that to change, and then if it disconnects, reconnect. Repeat.
            // If it's disconnected, wait a bit before trying again. Repeat.
            loop {
                let state = if let Some(task) = sent.upgrade() {
                    *task.lock().await.state.lock().await
                } else { break };

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
                        if let Some(instance) = sent.upgrade() {
                            if let Err(error) = Self::try_connect_in_place(
                                instance,
                                CONNECTION_ATTEMPT_TIMEOUT
                            ).await {
                                tracing::debug!(?error, "couldn't connect")
                            }
                        } else { break }
                    }
                }
            }
        });

        if let Some(instance) = instance.upgrade() {
            instance.lock().await.auto_reconnect_task_handle = Some(auto_reconnect_task_handle);
        }
    }


    pub async fn client(&mut self) -> Option<&mut discord_presence::Client> {
        match *self.state.try_lock().unwrap() {
            DiscordPresenceState::Ready => self.client.as_mut(),
            DiscordPresenceState::Disconnected => None
        }
    }

    /// Returns whether the status was cleared.
    /// (If the status was already empty, it will return false.)
    #[tracing::instrument(skip(self), level = "debug")]
    pub async fn clear(&mut self) -> Result<bool, UpdateError> {
        let has_content = self.has_content;
        if let Some(client) = self.client().await {
            if has_content {
                client.clear_activity()?;
                self.has_content = false;
            }
            Ok(has_content)
        } else if !has_content {
            Ok(false)
        } else {
            Err(UpdateError::NotConnected)
        }
    }

    #[tracing::instrument(skip(self), level = "debug")]
    async fn send_activity(&mut self) -> Result<(), DispatchError> {
        let activity = if let Some(activity) = self.activity.clone() { activity } else {
            return Err(DispatchError::internal_msg("no activity to dispatch", false))
        };

        let client = if let Some(client) = self.client.as_mut() { client } else {
            return Err(DispatchError::internal_msg("cannot dispatch without client", true))
        };

        client.set_activity(|_| activity.timestamps(|mut activity| {
            if let Some(position) = self.position {
                let start = chrono::Utc::now().timestamp() as u64 - position as u64;
                activity = activity.start(start);
                if let Some(duration) = self.duration {
                    activity = activity.end(start + duration as u64);
                }
            } 
            activity
        }))
            .map(|_| { self.has_content = true; })
            .map_err(|err| {
                use super::error::dispatch::{Recovery, RecoveryAttributes};
                use discord_presence::DiscordError;
                match err {
                    DiscordError::JsonError(err) => err.into(),
                    _ => DispatchError::internal(Box::new(err), Recovery::Continue(RecoveryAttributes {
                        log: Some(tracing::Level::WARN),
                        defer: false,
                    }))
                }
            })
    }

    /// Because of the ratelimit on Discord's end, it's sometimes not worth dispatching a length change
    /// if the track is about to change, as it'll delay the status update containing the new track.
    /// 
    /// This also updates the duration and position fields based on the new context.
    async fn should_dispatch_progress_update(&mut self, context: &super::BackendContext<()>) -> bool {
        const STATUS_UPDATE_RATELIMIT_SECONDS: f32 = 15.;
        self.duration = context.track.duration.map(|d| d.as_secs_f32());
        self.position = context.listened.lock().await.current.as_ref().map(|c| c.get_expected_song_position());
        let duration = if let Some(duration) = self.duration { duration } else { return true };
        let position = if let Some(position) = self.position { position } else { return true };
        let remaining = duration - position;
        remaining > (STATUS_UPDATE_RATELIMIT_SECONDS / 3. * 2.)
    }
}
impl Drop for DiscordPresence {
    fn drop(&mut self) {
        self.state_update_task_handle.abort();
        if let Some(handle) = self.auto_reconnect_task_handle.as_ref() {
            handle.abort();
        }
        if let Some(mut client) = self.client.take() {
            let _ = client.clear_activity();
            let _ = client.shutdown();
        }
    }
}

super::subscribe!(DiscordPresence, TrackStarted, {
    async fn get_solicitation(&self) -> ComponentSolicitation {
        let mut solicitation: ComponentSolicitation = ComponentSolicitation::default();
        solicitation.list.insert(Component::ITunesData);
        solicitation.list.insert(Component::AlbumImage);
        solicitation.list.insert(Component::ArtistImage);
        solicitation
    }

    async fn dispatch(&mut self, context: super::BackendContext<crate::data_fetching::AdditionalTrackData>) -> Result<(), DispatchError> {
        use osa_apple_music::track::MediaKind;
        let super::BackendContext { track, listened, data: additional_info, .. } = context;
        self.position = listened.lock().await.current.as_ref().map(|position| position.get_expected_song_position());
        self.duration = track.duration.map(|d| d.as_secs_f32());

        fn make_minimum_length(mut s: String) -> String {
            if s.len() < 2 {
                s += "  "; // two spaces
            }
            s
        }

        let mut activity = Activity::new()
            ._type(match track.media_kind {
                MediaKind::Song | 
                MediaKind::Unknown => ActivityType::Listening,
                MediaKind::MusicVideo => ActivityType::Watching,
            })
            .details(make_minimum_length(track.name.clone()))
            .state(track.artist.clone().map(make_minimum_length).unwrap_or("Unknown Artist".to_owned()))
            .assets(|_| ActivityAssets {
                large_text: track.album.clone().map(make_minimum_length),
                large_image: additional_info.images.track.clone(),
                small_image: additional_info.images.artist.clone(),
                small_text: track.artist.clone().map(make_minimum_length),
            });


        if let Some(itunes) = &additional_info.itunes {
            activity = activity.append_buttons(|button| button
                .label("Listen on Apple Music")
                .url(itunes.apple_music_url.clone())
            )
        }

        self.activity = Some(activity);
        self.send_activity().await
    }

});
super::subscribe!(DiscordPresence, ProgressJolt, {
    async fn dispatch(&mut self, context: super::BackendContext<()>) -> Result<(), DispatchError> {
        if self.should_dispatch_progress_update(&context).await {
            self.send_activity().await
        } else {
            tracing::debug!("skipping progress dispatch since it'll delay next song dispatch");
            Ok(())
        }
        // Err(DispatchError::internal_msg("not implemented", false))
    }
});

