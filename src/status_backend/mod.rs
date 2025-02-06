use std::{sync::Arc, time::Duration};
use tokio::{sync::Mutex, time::Instant};
use apple_music::{Track, ApplicationData};

use crate::data_fetching::components::ComponentSolicitation;

#[cfg(feature = "listenbrainz")]
pub mod listenbrainz;
#[cfg(feature = "lastfm")]
pub mod lastfm;
#[cfg(feature = "discord")]
pub mod discord;

#[async_trait::async_trait]
pub trait StatusBackend: core::fmt::Debug + Send + Sync {
    async fn set_now_listening(&mut self, track: Arc<Track>, app: Arc<ApplicationData>, data: Arc<crate::data_fetching::AdditionalTrackData>);
    async fn record_as_listened(&self, track: Arc<Track>, app: Arc<ApplicationData>);
    async fn check_eligibility(&self, track: Arc<Track>, time_listened: &Duration) -> bool;
    async fn get_additional_data_solicitation(&self) -> ComponentSolicitation {
        ComponentSolicitation::default()
    }
}

macro_rules! if_any_backend {
    ($op: tt, $($token: tt)*) => {
        #[cfg($op(any(
            feature = "discord",
            feature = "lastfm",
            feature = "listenbrainz",
        )))]
        { $($token)* }
    };
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
        { set = set.entry(&"DiscordPresence") }
        #[cfg(feature = "lastfm")]
        { set = set.entry(&"LastFM") }
        #[cfg(feature = "listenbrainz")]
        { set = set.entry(&"ListenBrainz") }

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
        };

        add!([
            (discord, "discord"),
            (lastfm, "lastfm"),
            (listenbrainz, "listenbrainz"),
        ]);

        backends
    }

    #[tracing::instrument(level = "debug")]
    pub async fn get_solicitations(&self) -> ComponentSolicitation {
        use self::StatusBackend;
        let mut solicitation = ComponentSolicitation::default();
        for backend in self.all() {
            // these don't really actually yield for anything
            solicitation += backend.lock().await.get_additional_data_solicitation().await;
        }
        solicitation
    }

    
    #[tracing::instrument(level = "debug")]
    pub async fn dispatch_track_ended(&self, track: Arc<Track>, app: Arc<ApplicationData>, elapsed: Duration) {
        let backends = self.all();
        let mut jobs = Vec::with_capacity(backends.len());

        for backend in backends {
            let track = track.clone();
            let app = app.clone();
            jobs.push(tokio::spawn(async move {
                if backend.lock().await.check_eligibility(track.clone(), &elapsed).await {
                    backend.lock().await.record_as_listened(track, app).await;
                }
            }));
        }

        for job in jobs {
            job.await.unwrap();
        }
    }

    #[tracing::instrument(level = "debug")]
    pub async fn dispatch_track_started(&self, track: Arc<Track>, app: Arc<ApplicationData>, data: Arc<crate::data_fetching::AdditionalTrackData>) {
        let backends = self.all();
        let mut jobs = Vec::with_capacity(backends.len());

        for mut backend in backends {
            let track = track.clone();
            let app = app.clone();
            let data = data.clone();
            jobs.push(tokio::spawn(async move {
                backend.lock().await.set_now_listening(track, app, data).await
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