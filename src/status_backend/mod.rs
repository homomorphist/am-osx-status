use std::{sync::Arc, time::{Duration, Instant}};
use tokio::sync::Mutex;

use crate::data_fetching::components::ComponentSolicitation;

use chrono::TimeDelta;
type DateTime = chrono::DateTime<chrono::Utc>;


trait TimeDeltaExtension {
    fn from_secs_f32(secs: f32) -> Self;
    fn as_secs_f32(&self) -> f32;
    fn as_secs_f64(&self) -> f64;
}
impl TimeDeltaExtension for TimeDelta {
    fn from_secs_f32(secs: f32) -> Self {
        let seconds = secs.trunc() as i64;
        let nanoseconds = (secs.fract() * 1e9) as u32;
        TimeDelta::new(seconds, nanoseconds).expect("bad duration")
    }
    fn as_secs_f32(&self) -> f32 {
        self.num_microseconds().expect("duration overflow") as f32 / 1e6
    }
    fn as_secs_f64(&self) -> f64 {
        self.num_microseconds().expect("duration overflow") as f64 / 1e6
    }
}

// const fn extract_delta_seconds_f32(delta: chrono::TimeDelta) -> f32 {
//     delta.num_microseconds().expect("duration overflow") as f32 / 1e6
// }

// fn delta_to_duration(delta: chrono::TimeDelta) -> Duration {
//     let u64: u64 = delta.num_microseconds().expect("duration overflow").try_into().expect("duration is negative");
//     Duration::from_micros(u64)}
// }


#[cfg(feature = "listenbrainz")]
pub mod listenbrainz;
#[cfg(feature = "lastfm")]
pub mod lastfm;
#[cfg(feature = "discord")]
pub mod discord;

#[derive(Debug)]
pub struct ListenedChunk {
    started_at_song_position: f32, // seconds
    started_at: DateTime,
    duration: chrono::TimeDelta 
}
impl ListenedChunk {
    pub fn ended_at(&self) -> DateTime {
        self.started_at.checked_add_signed(self.duration).expect("date out of range")
    }
    pub fn ended_at_song_position(&self) -> f32 {
        self.started_at_song_position + self.duration.as_secs_f32()
    }
}

#[derive(Debug, Clone)]
pub struct CurrentListened {
    started_at_song_position: f32, // seconds
    started_at: DateTime,
}
impl From<CurrentListened> for ListenedChunk {
    fn from(value: CurrentListened) -> Self {
        ListenedChunk {
            started_at: value.started_at,
            started_at_song_position: value.started_at_song_position,
            duration: chrono::Utc::now().signed_duration_since(value.started_at),
        }
    }
}
impl CurrentListened {
    pub fn new_with_position(position: f32) -> Self {
        Self {
            started_at: chrono::Utc::now(),
            started_at_song_position: position
        }
    }
    pub fn get_expected_song_position(&self) -> f32 {
        self.started_at_song_position + chrono::Utc::now().signed_duration_since(self.started_at).as_secs_f32()
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

    pub fn new_with_current(position: f32) -> Self {
        Self {
            contiguous: vec![],
            current: Some(CurrentListened::new_with_position(position)),
        }
    }

    pub fn started_at(&self) -> Option<DateTime> {
        self.current.as_ref().map(|c| c.started_at)
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
    
    pub fn set_new_current(&mut self, current_song_position: f32) {
        if self.current.replace(CurrentListened::new_with_position(current_song_position)).is_some() {
            tracing::warn!("overwrote current before it was flushed")
        }
    }
    
    pub fn total_heard_unique(&self) -> chrono::TimeDelta {
        if self.contiguous.is_empty() {
            return self.current.as_ref()
                .map(|current| chrono::Utc::now().signed_duration_since(current.started_at))
                .unwrap_or_default()
        }
        
        let mut total = chrono::TimeDelta::zero();
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
                let len = chunk_end - chunk_start.max(last_end_position);
                
                total += chrono::TimeDelta::new(len.trunc() as i64, (len.fract() * 1e6) as u32).expect("bad duration");
                last_end_position = chunk_end;
            }
        }

        total
    }

    pub fn total_heard(&self) -> chrono::TimeDelta {
        self.contiguous.iter()
            .map(|d| d.duration)
            .fold(
                self.current.as_ref()
                    .map(|c| chrono::Utc::now().signed_duration_since(c.started_at))
                    .unwrap_or_default(),
                |a, b| a + b
            )
    }
}


#[derive(Debug)]
pub struct BackendContext<A> {
    pub track: Arc<osa_apple_music::Track>,
    pub app: Arc<osa_apple_music::ApplicationData>,
    pub data: Arc<A>,
    pub listened: Arc<Mutex<Listened>>,
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
            let weak = Arc::downgrade(&wrapped);
            DiscordPresence::enable_auto_reconnect(weak).await;
            Some(wrapped)
        } else { None };

        StatusBackends {
            #[cfg(feature = "lastfm")] lastfm,
            #[cfg(feature = "discord")] discord,
            #[cfg(feature = "listenbrainz")] listenbrainz
        }
    }
}