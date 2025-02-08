#![allow(unused)]
use std::{process::ExitCode, sync::{Arc, atomic::AtomicBool}, time::{Duration, Instant}};
use config::{ConfigPathChoice, ConfigRetrievalError};
use musicdb::MusicDB;
use tracing::Instrument;

mod status_backend;
mod debugging;
mod data_fetching;
mod config;
mod service;
mod cli;
mod util;

fn watch_for_termination() -> (
    Arc<std::sync::atomic::AtomicBool>,
    std::pin::Pin<Box<impl std::future::Future<Output = tokio::signal::unix::SignalKind>>>
) {
    use tokio::signal::unix::{SignalKind, signal};
    use std::sync::atomic::{AtomicBool, Ordering};
    let flag = Arc::new(AtomicBool::new(false));
    let mut set = tokio::task::JoinSet::new();
    for kind in [
        SignalKind::quit(),
        SignalKind::hangup(),
        SignalKind::interrupt(),
        SignalKind::terminate(),
    ] {
        let mut sig = signal(kind).unwrap();
        let sent = flag.clone();
        set.spawn(async move {
            sig.recv().await;
            sent.store(true, Ordering::Relaxed);
            kind
        });
    }
    (
        flag,
        Box::pin(async move { set.join_next().await.unwrap().unwrap() })
    )
}

#[tokio::main(worker_threads = 4)]
async fn main() -> ExitCode {
    let args = <cli::Cli as clap::Parser>::parse();
    let config = config::Config::get(&args).await;
    let _ = debugging::DebuggingSession::new(&args);
    let (term, pending_term) = watch_for_termination();

    macro_rules! get_config_or_path {
        () => {
            match config {
                Ok(config) => Ok(config),
                Err(error) => match error {
                    ConfigRetrievalError::UnknownFs { inner, .. } => util::ferror!("could not read config: {inner}"),
                    ConfigRetrievalError::DeserializationFailure { inner, .. } => util::ferror!("could not read config: deserialization failure: {inner}"),
                    ConfigRetrievalError::PermissionDenied(path) => util::ferror!("could not read config: lacking permission to read {}", path.to_string_lossy()),
                    ConfigRetrievalError::NotFound(path) => { Err(path) }
                }
            }
        }
    }

    macro_rules! get_config_or_error {
        () => {
            get_config_or_path!().unwrap_or_else(|path| util::ferror!("no configuration file @ {}", path.to_string_lossy()))
        }
    }

    use cli::Command;
    match args.command {
        Command::Start => {
            let mut config = match get_config_or_path!() {
                Ok(config) => config,
                Err(path) => if config::wizard::io::prompt_bool(match path {
                    ConfigPathChoice::Automatic(..) => "No configuration has been set up! Would you like to use the wizard to build one?",
                    ConfigPathChoice::Explicit(..) => "No configuration exists at the provided file! Would you like to use the wizard to build it?",
                    ConfigPathChoice::Environmental(..) => "No configuration exists at the file specified in the environmental variable! Would you like to use the wizard to build it?",
                }) { config::Config::create_with_wizard(path).await } else {
                    println!("Proceeding with a temporary default configuration.");
                    config::Config::default()
                }
            };

            config.setup_side_effects().await;

            let backends = status_backend::StatusBackends::new(&config).await;
            let mut context = PollingContext::new(backends, Arc::clone(&term));

            // If we get stuck somewhere in the main loop, we still want a way to exit if the user/system desires.
            tokio::spawn(async {
                pending_term.await;
                tokio::time::sleep(Duration::new(1, 0)).await;
                std::process::exit(1);
            });

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
            tokio::spawn(async {
                pending_term.await;
                std::process::exit(1);
            });

            use cli::{ConfigurationAction, DiscordConfigurationAction};

            fn inform_whether_daemon_will_update(was_watching: bool) {
                let daemon_exists = service::ServiceController::new().is_program_active();
                if daemon_exists {
                    if was_watching {
                        // TODO: Only send this if they're using the same one that was being modified.
                        println!("The active service process will automatically adjust itself if it is configured by this file.");
                    } else {
                        // TODO: IPC
                        println!("The active service process is not configured to watch for file changes, it will need to be manually restarted.")
                    }
                }
            }

            match action {
                ConfigurationAction::Where => {
                    match config {
                        Ok(config) => {
                            println!("{}", config.path.to_string_lossy());
                            println!("this path was {}", config.path.describe_for_choice_reasoning_suffix());
                        },
                        Err(err) => {
                            use std::borrow::Cow;
                            let path = err.path();
                            println!("{}", path.to_string_lossy());
                            eprintln!("this path was {} but {}", path.describe_for_choice_reasoning_suffix(), match err {
                                ConfigRetrievalError::DeserializationFailure { .. } => Cow::Borrowed("it couldn't be successfully deserialized"),
                                ConfigRetrievalError::NotFound { .. } => Cow::Borrowed(if path.was_auto() { "it currently doesn't exist" } else { "it couldn't be found" }),
                                ConfigRetrievalError::PermissionDenied(_) => Cow::Borrowed("the required permissions to read it are not available"),
                                ConfigRetrievalError::UnknownFs { inner, .. } => Cow::Owned(format!("an unknown error occurred trying to read it ({})", inner))
                            })
                        },
                    }
                },
                ConfigurationAction::Wizard => {
                    match get_config_or_path!() {
                        Err(path) => {
                            println!("Creating configuration file @ {}", path.to_string_lossy());
                            let config = config::Config::create_with_wizard(path).await;
                            config.save_to_disk().await;
                            println!("Successfully saved changes!");
                        }
                        Ok(mut config) => {
                            let was_watching = config.watch_config_file;
                            println!("Modifying configuration file @ {}", config.path.to_string_lossy());
                            config.edit_with_wizard().await;
                            config.save_to_disk().await;
                            println!("Successfully saved changes!");
                            inform_whether_daemon_will_update(was_watching)
                        },
                    }
                },
                ConfigurationAction::Discord { action } => {
                    let mut config = get_config_or_error!();
                    match action {
                        DiscordConfigurationAction::Enable => config.backends.discord = true,
                        DiscordConfigurationAction::Disable => config.backends.discord = false
                    };
                    config.save_to_disk().await;
                    inform_whether_daemon_will_update(config.watch_config_file);
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
                if let Err(error) = presence.lock().await.clear().await {
                    tracing::error!(?error, "unable to clear discord status")
                }
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
                if let Err(error) = presence.lock().await.clear().await {
                    tracing::error!(?error, "unable to clear discord status")
                }
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
