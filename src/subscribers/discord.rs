use std::{fmt::Debug, sync::{Arc, Weak}, time::Duration};
use discord_presence::models::{Activity, ActivityAssets, ActivityType};
use tokio::sync::Mutex;

use crate::data_fetching::components::{Component, ComponentSolicitation};

use super::error::DispatchError;

macro_rules! define_activities {
    (
        $(#[$meta: meta])*
        $vis: vis
        $name: ident {
            $(
                $(#[$activity_meta: meta])*
                $activity: ident = $num_id: literal # $display: literal
            ),*,
        }
    ) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[repr(u64)]
        $(#[$meta])*
        $vis enum $name {
            $(
                $(#[$activity_meta])*
                $activity = $num_id
            ),*
        }

        impl $name {
            #[allow(clippy::cast_enum_truncation)]
            pub const VARIANT_COUNT: usize = {
                0 + $({
                    1 - {
                        // No-op: simply for correct repetition.
                        $name::$activity as usize - $name::$activity as usize
                    }
                } +)* 0
            };

            pub const VARIANTS: [Self; Self::VARIANT_COUNT] = [
                $(
                    $name::$activity,
                )*
            ];

            pub const fn get_display_text(self) -> &'static str {
                match self {
                    $($name::$activity => $display),*,
                }
            }
            pub const fn get_id(self) -> u64 {
                match self {
                    $($name::$activity => $num_id),*,
                }
            }

            pub fn default_as_u64() -> u64 {
                Self::default() as u64
            }
        }
    };
}
define_activities! {
    #[derive(Default)]
    pub EnumeratedApplicationIdentifier {
        #[default]
        AppleMusic     = 1286481105410588672 # "Apple Music",
        Music          = 1376721849622335519 #       "Music",
        MusicLowercase = 1376721968874782731 #       "music",
    }
}
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy)]
pub struct Config {
    pub enabled: bool,
    #[serde(default = "EnumeratedApplicationIdentifier::default_as_u64")]
    pub application_id: u64,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            application_id: EnumeratedApplicationIdentifier::default_as_u64(),
        }
    }
}

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

use core::sync::atomic::Ordering;

/// A cancelable status clear that will occur after [`THRESHOLD`](Self::THRESHOLD) amount of time.
/// 
/// A [`Pause`](super::subscription::Pause) can get dispatched during
/// not just deliberate user intent, but also the time when waiting
/// for a song to load.
/// If we immediately clear the status when this occurs, the ratelimit
/// imposed by Discord means that it will take some time before the
/// status gets correctly set to the new song.
/// As such, a delay is used to ensure that the pause lasts a reasonable
/// duration that is unlikely to just be buffering or a quick pause before
/// making the decision to clear the status.
#[derive(Default)]
struct PendingStatusClear {
    intent: Arc<core::sync::atomic::AtomicBool>,
    cancel: Arc<tokio::sync::Notify>,
    pub act: Arc<tokio::sync::Notify> // hook
}
impl PendingStatusClear {
    /// The amount of time a pause needs to last for before the status is cleared.
    pub const THRESHOLD: core::time::Duration = core::time::Duration::from_secs(5);

    fn signal(&mut self) {
        if self.intent.compare_exchange(
            false,
            true,
            Ordering::SeqCst,
            Ordering::Relaxed,
        ).is_err() {
            return;
        }

        let act = self.act.clone();
        let cancel = self.cancel.clone();
        let intends_to_clear = self.intent.clone();
        tokio::spawn(async move {
            let sleep = tokio::time::sleep(Self::THRESHOLD);
            let cancelled = cancel.notified();
            tokio::select!{
                _ = cancelled => {},
                _ = sleep => {
                    if intends_to_clear.compare_exchange(
                        true,
                        false,
                        Ordering::SeqCst,
                        Ordering::Relaxed
                    ).is_ok() {
                        act.notify_waiters();
                    }
                }
            }
        });
    }

    fn cancel(&mut self) {
        self.intent.store(false, Ordering::Relaxed);
        self.cancel.clone().notify_waiters();
    }
}


const CONNECTION_ATTEMPT_TIMEOUT: tokio::time::Duration = tokio::time::Duration::from_secs(3);
const TRY_AGAIN_DEBOUNCE: tokio::time::Duration = tokio::time::Duration::from_secs(7);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DiscordPresenceState {
    Ready,
    Disconnected,
}

super::subscription::define_subscriber!(pub DiscordPresence, {
    config: Config,
    client: Option<discord_presence::Client>,
    state: Arc<Mutex<DiscordPresenceState>>,
    state_channel: tokio::sync::broadcast::Sender<DiscordPresenceState>,
    state_update_task_handle: tokio::task::JoinHandle<()>,
    auto_reconnect_task_handle: Option<tokio::task::JoinHandle<()>>,
    has_content: bool,
    activity: Option<discord_presence::models::Activity>,
    position: Option<f32>,
    duration: Option<f32>,
    pending_clear: PendingStatusClear,
});
impl Debug for DiscordPresence {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct(Self::NAME).finish()
    }
}
impl DiscordPresence {
    #[tracing::instrument(level = "debug")]
    pub async fn new(config: Config) -> Arc<Mutex<Self>> {
        let instance = Self::disconnected(config).await;
        match DiscordPresence::try_connect_in_place(instance.clone(), CONNECTION_ATTEMPT_TIMEOUT).await {
            Ok(()) => instance,
            Err(ConnectError::TimedOut) => {
                tracing::warn!("client creation timed out; assuming Discord isn't open");
                Self::disconnected(config).await
            }
        }
    }

    pub async fn disconnected(config: Config) -> Arc<Mutex<Self>> {
        let (tx, mut rx) = tokio::sync::broadcast::channel(4);

        let state = Arc::new(Mutex::new(DiscordPresenceState::Disconnected));
        let update_v = state.clone();
        let state_update_hook = tokio::spawn(async move {
            loop {
                let state = rx.recv().await.expect("channel closed: all senders were dropped");
                *update_v.try_lock().unwrap() = state;
            }
        });

        let pending_clear = PendingStatusClear::default();
        let pending_clear_act = pending_clear.act.clone();
        let this = Arc::new(Mutex::new(Self {
            config,
            client: None,
            state,
            state_channel: tx,
            state_update_task_handle: state_update_hook,
            auto_reconnect_task_handle: None,
            has_content: false,
            activity: None,
            position: None,
            duration: None,
            pending_clear,
        }));

        let weak = Arc::downgrade(&this);
        DiscordPresence::enable_auto_reconnect(weak.clone()).await;
        DiscordPresence::react_to_pending_clear(weak, pending_clear_act);

        this
    }

    pub async fn connect(mut self) -> Self {
        self.connect_in_place().await;
        self
    }

    /// not `tokio::select!` safe
    #[tracing::instrument(skip(self), level = "debug")]
    pub async fn connect_in_place(&mut self) {
        let client = discord_presence::Client::new(self.config.application_id);            
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


    async fn enable_auto_reconnect(weak: Weak<Mutex<Self>>) {
        let mut rx = if let Some(instance) = weak.upgrade() {
            let lock = instance.lock().await;

            if let Some(old_handle) = &lock.auto_reconnect_task_handle {
                old_handle.abort();
            };
            
            lock.state_channel.subscribe()
        } else { return };

        let sent = weak.clone();

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

        if let Some(instance) = weak.upgrade() {
            instance.lock().await.auto_reconnect_task_handle = Some(auto_reconnect_task_handle);
        }
    }

    fn react_to_pending_clear(instance: Weak<Mutex<Self>>, signal: Arc<tokio::sync::Notify>) {
       tokio::spawn(async move {
            loop {
                signal.notified().await;
                if let Some(this) = instance.upgrade() {
                    if let Err(error) = this.lock().await.clear().await {
                        tracing::error!(?error, "unable to clear discord status")
                    }
                } else {
                    tracing::debug!("discord presence instance was dropped, stopping pending clear task");
                    break;
                }
            }
        });
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
        let activity = self.activity.clone().ok_or(DispatchError::internal_msg("no activity to dispatch", false))?;
        let client = self.client.as_mut().ok_or(DispatchError::internal_msg("cannot dispatch without client", true))?;

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
        let image_urls = additional_info.images.urls();

        fn make_minimum_length(mut s: String) -> String {
            if s.len() < 2 {
                s += "  "; // two spaces
            }
            s
        }

        let mut activity = Activity::new()
            .activity_type(match track.media_kind {
                MediaKind::Song | 
                MediaKind::Unknown => ActivityType::Listening,
                MediaKind::MusicVideo => ActivityType::Watching,
            })
            .details(make_minimum_length(track.name.clone()))
            .state(track.artist.clone().map(make_minimum_length).unwrap_or("Unknown Artist".to_owned()))
            .assets(|_| ActivityAssets {
                large_text: track.album.clone().map(make_minimum_length),
                large_image: image_urls.track.map(str::to_owned),
                small_image: image_urls.artist.map(str::to_owned),
                small_text: track.artist.clone().map(make_minimum_length),
            });

        if let Some(itunes) = &additional_info.itunes {
            activity = activity
                .append_buttons(|button| button
                    .label("Check it out!")
                    .url(format!("https://song.link/{}&app=music", itunes.apple_music_url))
                );
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
    }
});
super::subscribe!(DiscordPresence, ApplicationStatusUpdate, {
    async fn dispatch(&mut self, status: super::DispatchedApplicationStatus) -> Result<(), DispatchError> {
        use super::DispatchedApplicationStatus;
        match status != DispatchedApplicationStatus::Playing {
            true  => self.pending_clear.signal(),
            false => self.pending_clear.cancel(),
        };
        Ok(())
    }
});



