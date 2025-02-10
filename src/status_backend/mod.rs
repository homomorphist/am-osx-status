use std::{sync::Arc, time::{Duration, Instant}};
use tokio::sync::Mutex;
use apple_music::{Track, ApplicationData};

use crate::data_fetching::components::ComponentSolicitation;

#[cfg(feature = "listenbrainz")]
pub mod listenbrainz;
#[cfg(feature = "lastfm")]
pub mod lastfm;
#[cfg(feature = "discord")]
pub mod discord;

#[derive(Debug)]
pub struct ListenedChunk {
    started_at_song_position: f64, // seconds
    started_at: Instant,
    duration: Duration 
}
impl ListenedChunk {
    pub fn ended_at(&self) -> Instant {
        self.started_at.checked_add(self.duration).unwrap()
    }
    pub const fn ended_at_song_position(&self) -> f64 {
        self.started_at_song_position + self.duration.as_secs_f64()
    }
}

#[derive(Debug, Clone)]
pub struct CurrentListened {
    started_at_song_position: f64, // seconds
    started_at: Instant,
}
impl From<CurrentListened> for ListenedChunk {
    fn from(value: CurrentListened) -> Self {
        ListenedChunk {
            started_at: value.started_at,
            started_at_song_position: value.started_at_song_position,
            duration: Instant::now().duration_since(value.started_at)
        }
    }
}
impl CurrentListened {
    pub fn new_with_position(position: f64) -> Self {
        Self {
            started_at: Instant::now(),
            started_at_song_position: position
        }
    }
    pub fn get_expected_song_position(&self) -> f64 {
        self.started_at_song_position + Instant::now().duration_since(self.started_at).as_secs_f64()
    }
}

#[derive(Debug, Default)]
pub struct Listened {
    pub contiguous: Vec<ListenedChunk>,
    pub current: Option<CurrentListened>,
}
impl Listened {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_with_current(position: f64) -> Self {
        Self {
            contiguous: vec![],
            current: Some(CurrentListened::new_with_position(position)),
        }
    }

    fn find_index_for_current(&self, current: &CurrentListened) -> usize {
        self.contiguous.iter()
            .enumerate()
            .filter(|(_, chunk)| chunk.started_at_song_position < current.started_at_song_position)
            .last().map(|(i, _)| i + 1).unwrap_or_default()
    }

    pub fn flush_current(&mut self) {
        if let Some(current) = self.current.take() {
            let index = self.find_index_for_current(&current);
            self.contiguous.insert(index, current.into());
        }
    }
    
    pub fn set_new_current(&mut self, current_song_position: f64) {
        if self.current.replace(CurrentListened::new_with_position(current_song_position)).is_some() {
            tracing::warn!("overwrote current before it was flushed")
        }
    }
    
    pub fn total_heard_unique(&self) -> Duration {
        if self.contiguous.is_empty() {
            return self.current.as_ref()
                .map(|current| Instant::now().duration_since(current.started_at))
                .unwrap_or_default()
        }
        
        let mut total = Duration::new(0, 0);
        let mut last_end_position = 0.0;

        let current = self.current.clone().map(|current| (
            self.find_index_for_current(&current),
            Into::<ListenedChunk>::into(current),
        ));
        
        for mut index in 0..self.contiguous.len() + if current.is_some() { 1 } else { 0 } {
            let chunk = if let Some((current_idx, current)) = &current {
                use core::cmp::Ordering;
                match index.cmp(current_idx) {
                    Ordering::Greater => &self.contiguous[index - 1],
                    Ordering::Equal => current,
                    Ordering::Less => &self.contiguous[index]
                }
            } else { &self.contiguous[index] };

            let chunk_start = chunk.started_at_song_position;
            let chunk_end = chunk.ended_at_song_position();

            if chunk_end > last_end_position {
                total += Duration::from_secs_f64(chunk_end - chunk_start.max(last_end_position));
                last_end_position = chunk_end;
            }
        }

        total
    }

    pub fn total_heard(&self) -> Duration {
        self.contiguous.iter()
            .map(|d| d.duration)
            .fold(
                self.current.as_ref()
                    .map(|c| Instant::now().duration_since(c.started_at))
                    .unwrap_or_default(),
                |a, b| a + b
            )
    }
}


#[derive(Debug)]
pub struct BackendContext<A> {
    pub track: Arc<Track>,
    pub app: Arc<ApplicationData>,
    pub data: Arc<A>,
    pub listened: Arc<Mutex<Listened>>
}
impl<A> Clone for BackendContext<A> {
    fn clone(&self) -> Self {
        Self {
            track: self.track.clone(),
            app: self.app.clone(),
            data: self.data.clone(),
            listened: self.listened.clone(),
        }
    }
}

#[async_trait::async_trait]
pub trait StatusBackend: core::fmt::Debug + Send + Sync {
    async fn set_now_listening(&mut self, context: BackendContext<crate::data_fetching::AdditionalTrackData>);
    async fn record_as_listened(&self, context: BackendContext<()>);
    async fn check_eligibility(&self, context: BackendContext<()>) -> bool;
    async fn update_progress(&mut self, context: BackendContext<()>) {}
    async fn get_additional_data_solicitation(&self) -> ComponentSolicitation {
        ComponentSolicitation::default()
    }
}

pub struct StatusBackends {
    #[cfg(feature = "discord")]
    pub discord: Option<Arc<Mutex<discord::DiscordPresence>>>,
    #[cfg(feature = "lastfm")]
    pub lastfm: Option<Arc<Mutex<lastfm::LastFM>>>,
    #[cfg(feature = "listenbrainz")]
    pub listenbrainz: Option<Arc<Mutex<listenbrainz::ListenBrainz>>>
}
impl core::fmt::Debug for StatusBackends {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut set = &mut f.debug_set();

        #[cfg(feature = "discord")]
        { if self.discord.is_some() { set = set.entry(&"DiscordPresence") } }
        #[cfg(feature = "lastfm")]
        { if self.lastfm.is_some() { set = set.entry(&"LastFM") } }
        #[cfg(feature = "listenbrainz")]
        { if self.listenbrainz.is_some() { set = set.entry(&"ListenBrainz") } }

        set.finish()
    }
}

impl StatusBackends {
    pub fn all(&self) -> Vec<Arc<Mutex<dyn StatusBackend>>> {
        let mut backends: Vec<Arc<Mutex<dyn StatusBackend>>> = vec![];

        macro_rules! add {
            ([$(($property: ident, $feature: literal) $(,)?)*]) => {
                $(
                    #[cfg(feature = $feature)]
                    { if let Some(backend) = &self.$property { backends.push(backend.clone()) } }
                )*
            };
        }

        add!([
            (discord, "discord"),
            (lastfm, "lastfm"),
            (listenbrainz, "listenbrainz"),
        ]);

        backends
    }

    #[tracing::instrument(level = "debug")]
    pub async fn get_solicitations(&self) -> ComponentSolicitation {
        let mut solicitation = ComponentSolicitation::default();
        for backend in self.all() {
            // these don't really actually yield for anything
            solicitation += backend.lock().await.get_additional_data_solicitation().await;
        }
        solicitation
    }

    
    #[tracing::instrument(skip(context), level = "debug")]
    pub async fn dispatch_track_ended(&self, context: BackendContext<()>) {
        let backends = self.all();
        let mut jobs = Vec::with_capacity(backends.len());

        for backend in backends {
            let context = context.clone();
            jobs.push(tokio::spawn(async move {
                if backend.lock().await.check_eligibility(context.clone()).await {
                    backend.lock().await.record_as_listened(context).await;
                }
            }));
        }

        for job in jobs {
            job.await.unwrap();
        }
    }

    #[tracing::instrument(skip(context), level = "debug", fields(track = &context.track.persistent_id))]
    pub async fn dispatch_track_started(&self, context: BackendContext<crate::data_fetching::AdditionalTrackData>) {
        let backends = self.all();
        let mut jobs = Vec::with_capacity(backends.len());

        for backend in backends {
            let context = context.clone();
            jobs.push(tokio::spawn(async move {
                backend.lock().await.set_now_listening(context).await
            }));
        }

        for job in jobs {
            job.await.unwrap();
        }

    }

    #[tracing::instrument(skip(context), level = "debug")]
    pub async fn dispatch_current_progress(&self, context: BackendContext<()>) {
        let backends = self.all();
        let mut jobs = Vec::with_capacity(backends.len());

        for backend in backends {
            let context = context.clone();
            jobs.push(tokio::spawn(async move {
                backend.lock().await.update_progress(context).await;
            }));
        }

        for job in jobs {
            job.await.unwrap();
        }
    }

    pub async fn new(config: &crate::config::Config<'_>) -> StatusBackends {
        #[cfg(feature = "lastfm")]
        use crate::status_backend::lastfm::*;

        #[cfg(feature = "discord")]
        use crate::status_backend::discord::*;

        #[cfg(feature = "listenbrainz")]
        use crate::status_backend::listenbrainz::*;

        #[cfg(feature = "lastfm")]
        let lastfm = config.backends.lastfm.as_ref().and_then(|config| {
            if config.enabled {
                Some(Arc::new(Mutex::new(LastFM::new(
                    config.identity.clone(),
                    config.session_key.clone().expect("no session keys")
                ))))
            } else { None }
        });
        
        #[cfg(feature = "listenbrainz")]
        let listenbrainz = config.backends.listenbrainz.as_ref().and_then(|config| {
            if config.enabled {
                Some(Arc::new(Mutex::new(ListenBrainz::new(
                    config.program_info.clone(),
                    config.user_token.clone().expect("no token")
                ))))
            } else { None }
        });

        #[cfg(feature = "discord")]
        let discord = if config.backends.discord {
            let wrapped = Arc::new(Mutex::new(DiscordPresence::new().await));
            DiscordPresence::enable_auto_reconnect(wrapped.clone()).await;
            Some(wrapped)
        } else { None };

        StatusBackends {
            #[cfg(feature = "lastfm")] lastfm,
            #[cfg(feature = "discord")] discord,
            #[cfg(feature = "listenbrainz")] listenbrainz
        }
    }
}