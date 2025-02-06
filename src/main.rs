#![allow(unused)]
use std::{ops::DerefMut, process::ExitCode, sync::{atomic::AtomicBool, Arc}, time::{Duration, Instant}};
use discord_presence::Event;
use musicdb::MusicDB;
use tracing::Instrument;

mod status_backend;
mod debugging;
mod data_fetching;
mod config;
mod service;
mod cli;
mod util;

#[tokio::main(worker_threads = 4)]
async fn main() -> ExitCode {
    let args = <cli::Cli as clap::Parser>::parse();
    let mut config = config::Config::get(&args).await;
    let debugging = debugging::DebuggingSession::new(&config, &args);

    use cli::Command;
    match args.command {
        Command::Start => {
            let term = Arc::new(std::sync::atomic::AtomicBool::new(false));
            signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&term)).expect("cannot register sigterm hook");
            signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&term)).expect("cannot register sigint hook");

            let backends = status_backend::StatusBackends::new(&config).await;
            let mut context = PollingContext::new(backends, Arc::clone(&term));

            while !term.load(std::sync::atomic::Ordering::Relaxed) {
                proc_once(&mut context).await;
            }
        }
        Command::Service { action } => {
            use cli::ServiceAction;

            let manager = service::ServiceController::new();

            match action {
                ServiceAction::Start => manager.start(false).unwrap(),
                ServiceAction::Stop => match manager.stop().unwrap() {
                    0 | 1 => (),
                    n if n > 1 => println!("[!] Killed {} processes", n),
                    _ => unreachable!()
                },
                ServiceAction::Restart => manager.restart().unwrap(),
            };
        },
        Command::Configure { ref action } => {
            use cli::{ConfigurationAction, DiscordConfigurationAction};
            match action {
                ConfigurationAction::Where => println!("{}", config.path.as_path().to_string_lossy()),
                ConfigurationAction::Wizard => {
                    config = config::Config::create_with_wizard(config.path).await;
                    config.save_to_disk().await;
                    let service_controller = service::ServiceController::new();
                    if service_controller.is_program_active() {
                        // TODO: Watch the configuration file for changes using `kqueue`
                        println!("Changes have been saved, but will not take effect until the service is restarted");
                    } else {
                        println!("Changes have been saved.")
                    }

                },
                ConfigurationAction::Discord { action } => {
                    // action
                    todo!();
                }
            }
        }
    }

    ExitCode::SUCCESS
}

#[derive(Debug)]
struct PollingContext<'a> {
    terminating: Arc<AtomicBool>,
    backends: status_backend::StatusBackends,
    pub last_track: Option<Arc<apple_music::Track>>,
    pub last_track_started_at: Instant,
    custom_artwork_host: Option<Box<dyn data_fetching::services::custom_artwork_host::CustomArtworkHost>>,
    musicdb: Option<musicdb::MusicDB<'a>>
}
impl PollingContext<'_> {
    fn new(backends: status_backend::StatusBackends, terminating: Arc<AtomicBool>) -> Self {
        Self {
            terminating,
            backends,
            last_track: None,
            last_track_started_at: Instant::now(),
            custom_artwork_host: Some(Box::new(data_fetching::services::custom_artwork_host::catbox::CatboxHost::new())),
            musicdb: Some(MusicDB::default()),
        }
    }
}

#[tracing::instrument(skip(context))]
async fn proc_once(context: &mut PollingContext<'_>) {
    use apple_music::{AppleMusic, PlayerState, Track};

    // TODO: poll discord presence

    let app = match tracing::info_span!("app status retrieval").in_scope(AppleMusic::get_application_data) {
        Ok(app) => Arc::new(app),
        Err(err) => {
            use apple_music::Error;
            match &err {
                Error::DeserializationFailed if context.terminating.load(std::sync::atomic::Ordering::Relaxed) => { return } // child killed before us
                Error::DeserializationFailed | Error::NoData | Error::AppCommandFailed => { tracing::error!("{:?}", &err); return },
                Error::NotPlaying => { return }
            }
        }
    };

    match app.player_state.as_ref().expect("could not retrieve player state") {
        PlayerState::FastForwarding | PlayerState::Rewinding => unimplemented!(),
        PlayerState::Stopped => {
            #[cfg(feature = "discord")]
            if let Some(presence) = context.backends.discord.clone() {
                presence.lock().await.clear();
            }
            
            let now: Instant = Instant::now();
            let elapsed = now - context.last_track_started_at;
            
            if let Some(previous) = context.last_track.clone() {
                context.backends.dispatch_track_ended(previous, app.clone(), elapsed).await;
                context.last_track = None;
            }
        }
        PlayerState::Paused => {
            #[cfg(feature = "discord")]
            if let Some(presence) = context.backends.discord.clone() {
                presence.lock().await.clear();
            }
            
            context.last_track = None;
        },

        PlayerState::Playing => {
            let track = match tracing::info_span!("track retrieval").in_scope(AppleMusic::get_current_track) {
                Ok(track) => Arc::new(track),
                Err(err) => {
                    use apple_music::Error;
                    match &err {
                        Error::DeserializationFailed if context.terminating.load(std::sync::atomic::Ordering::Relaxed) => { return } // child killed before us
                        Error::DeserializationFailed | Error::NoData | Error::AppCommandFailed => { tracing::error!("{:?}", &err); return },
                        Error::NotPlaying => { return }
                    }
                }
            };

            
            let previous = context.last_track.as_ref().map(|v: &Arc<Track>| &v.persistent_id);
            if previous != Some(&track.persistent_id) {
                tracing::trace!("new track: {:?}", track);
                let now = Instant::now();
                let elapsed = now - context.last_track_started_at;
                
                use data_fetching::AdditionalTrackData;
                let solicitation = context.backends.get_solicitations().await;
                let additional_data_pending = AdditionalTrackData::from_solicitation(solicitation, &track, context.musicdb.as_ref(), context.custom_artwork_host.as_mut());
                let additional_data = if let Some(previous) = context.last_track.clone() {
                    let pending_dispatch = context.backends.dispatch_track_ended(previous, app.clone(), elapsed);
                    async move { 
                        // Run dispatch concurrently while we fetch the additional data for the next
                        tokio::join!(
                            additional_data_pending,
                            pending_dispatch.instrument(tracing::trace_span!("song end dispatch"))
                        )
                    }.await.0
                } else {
                    additional_data_pending.await
                };


                context.backends.dispatch_track_started(track.clone(), app, Arc::new(additional_data)).await;
                context.last_track = Some(track);
                context.last_track_started_at = now;
            }
        }
    }
}
