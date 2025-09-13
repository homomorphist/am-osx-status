#![allow(unused)]
use std::{ops::DerefMut, process::ExitCode, sync::{atomic::AtomicBool, Arc}, time::Duration};
use config::{ConfigPathChoice, ConfigRetrievalError};
use subscribers::{subscription, BackendContext, DispatchableTrack};
use tokio::sync::Mutex;
use tracing::Instrument;
use listened::Listened;
use util::ferror;

use crate::service::lockfile::ActiveProcessLockfile;
use crate::config::LoadableConfig;

mod subscribers;
mod listened;
mod debugging;
mod data_fetching;
mod service;
mod config;
mod cli;
mod util;
mod store;

const POLL_INTERVAL: Duration = Duration::from_millis(500);

type TerminationFuture = std::pin::Pin<Box<dyn std::future::Future<Output = tokio::signal::unix::SignalKind> + Send>>;

fn watch_for_termination() -> (
    Arc<std::sync::atomic::AtomicBool>,
    TerminationFuture,
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

#[tokio::main(worker_threads = 1)]
async fn main() -> ExitCode {
    let args = Box::leak(Box::new(<cli::Cli as clap::Parser>::parse()));
    let config = config::Config::get(args).await;
    let debugging = debugging::DebuggingSession::new(args);
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

    macro_rules! get_config_os_string {
        () => {
            std::ffi::OsString::from(&*match get_config_or_path!() {
                Ok(config) => config.path,
                Err(path) => path
            }.to_string_lossy())
        };
    }

    macro_rules! get_config_or_error {
        () => {
            get_config_or_path!().unwrap_or_else(|path| util::ferror!("no configuration file @ {}", path.to_string_lossy()))
        }
    }

    use cli::Command;
    match args.command {
        Command::Start => {
            if let Some(pid) = ActiveProcessLockfile::get().await {
                eprintln!("Another instance of the program is already running! (pid {pid})");

                if service::ServiceController::is_running().await {
                    eprintln!("You can turn off the service with `am-osx-status service stop`.");
                }

                return ExitCode::FAILURE;
            }

            ActiveProcessLockfile::write().await;

            let config = match get_config_or_path!() {
                Ok(config) => {
                    config.save_to_disk().await;
                    config
                },
                Err(path) => if config::wizard::io::prompt_bool(match path {
                    ConfigPathChoice::Automatic(..) => "No configuration has been set up! Would you like to use the wizard to build one?",
                    ConfigPathChoice::Explicit(..) => "No configuration exists at the provided file! Would you like to use the wizard to build it?",
                    ConfigPathChoice::Environmental(..) => "No configuration exists at the file specified in the environmental variable! Would you like to use the wizard to build it?",
                }) {
                    let config = config::Config::create_with_wizard(path).await;
                    config.save_to_disk().await;
                    println!("Configuration file has been saved.");
                    config
                } else {
                    println!("Proceeding with a temporary default configuration.");
                    config::Config::default()
                }
            };

            let context = Arc::new(Mutex::new(PollingContext::from_config(&config, Arc::clone(&term)).await));
            let config = Arc::new(Mutex::new(config));
            
            let listener = if args.running_as_service {
                Some(service::ipc::listen(
                    context.clone(),
                    config.clone()
                ).await)
            } else { None };

            // If we get stuck somewhere in the main loop, we still want a way to exit if the user/system desires.
            let session_closer_ctx = context.clone();
            tokio::spawn(async move {
                pending_term.await;
                if let Some(listener) = listener {
                   listener.abort();
                }
                drop(debugging.guards); // flush logs
                let db_pool = &store::DB_POOL.get().await.expect("failed to get database pool");
                let session = &mut session_closer_ctx.lock().await.session;
                session.ended_at = Some(chrono::Utc::now().into());
                let (finished, cleared) = tokio::join!(
                    session.finish(db_pool),
                    ActiveProcessLockfile::clear(),
                );
                finished.expect("failed to finish session in database");
                cleared.expect("failed to clear active process lockfile");
                std::process::exit(0)
            });


            tracing::info!("starting main loop");

            let mut interval = tokio::time::interval(POLL_INTERVAL);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

            while !term.load(std::sync::atomic::Ordering::Relaxed) {
                proc_once(context.clone()).await;
                interval.tick().await;
            }
        },
        Command::Service { ref action } => {
            use cli::ServiceAction;
            use service::*;

            #[derive(thiserror::Error, Debug)]
            enum ServiceIpcError {
                #[error("failed to dispatch IPC packet ({0})")]
                Dispatch(#[from] std::io::Error),
                #[error("failed to establish IPC connection ({0})")]
                Connection(std::io::Error),
            }

            match action {
                ServiceAction::Start => ServiceController::start(get_config_or_error!().path.as_path(), true).await,
                ServiceAction::Stop => ServiceController::stop(true).await,
                ServiceAction::Status => {
                    enum ServiceDefinitionStatus {
                        Installed,
                        NotInstalled,
                        Indeterminate(std::io::Error),
                    }

                    let status = match ServiceController::is_defined().await {
                        Ok(true) => ServiceDefinitionStatus::Installed,
                        Ok(false) => ServiceDefinitionStatus::NotInstalled,
                        Err(err) => ServiceDefinitionStatus::Indeterminate(err)
                    };

                    if let Some(pid) = ServiceController::pid().await {
                        println!("Service is running with PID {pid}.");
                        match status {
                            ServiceDefinitionStatus::Installed => {}
                            ServiceDefinitionStatus::NotInstalled => println!("The definition has since been removed, though, so it will not start automatically after shutdown."),
                            ServiceDefinitionStatus::Indeterminate(err) => println!("Could not determine if the service is installed: {err}"),
                        }   
                    } else if let Some(pid) = ActiveProcessLockfile::get().await {
                        println!("Service is not running, but an instance of the program is running independently with PID {pid}.");
                        match status {
                            ServiceDefinitionStatus::Installed => println!("It is installed and will start automatically on login, or can be manually started after the running instance is closed."),
                            ServiceDefinitionStatus::NotInstalled => println!("The service is not currently installed."),
                            ServiceDefinitionStatus::Indeterminate(err) => println!("Could not determine if the service is installed: {err}"),
                        }
                    } else {
                        print!("Service is not running");
                        match status {
                            ServiceDefinitionStatus::Installed => println!(", but it is installed and will start automatically on login."),
                            ServiceDefinitionStatus::NotInstalled => println!(" and is not installed."),
                            ServiceDefinitionStatus::Indeterminate(err) => println!(".\nCould not determine if it is installed: {err}"),
                        }
                    }
                },
                ServiceAction::Restart => ServiceController::restart(get_config_or_error!().path.as_path()).await,
                ServiceAction::Remove => ServiceController::remove().await,
                ServiceAction::Reload => {
                    use ipc::{Packet, PacketConnection};
                    let path = get_config_or_error!().socket_path;
                    let mut connection = dbg!(PacketConnection::from_path(path).await).unwrap();
                    connection.send(Packet::hello()).await;
                    connection.send(Packet::ReloadConfiguration).await;
                    println!("Reload command sent to service.");
                }
            }
        },
        Command::Configure { ref action } => {
            tokio::spawn(async {
                pending_term.await;
                std::process::exit(0);
            });

            use cli::ConfigurationAction;

            match action {
                ConfigurationAction::Where { show_reason, escape} => {
                    let path = match &config {
                        Ok(config) => &config.path,
                        Err(error) => error.path()
                    };

                    let path_str = path.to_string_lossy();
                    let path_str = if !escape { path_str } else {
                        String::from(path_str)
                            .replace(' ', "\\ ")
                            .into()
                    };

                    use std::io::IsTerminal;
                    let show_reason = match show_reason {
                        Some(show) => *show,
                        None => std::io::stdout().is_terminal()
                    };

                    println!("{path_str}");
                    if show_reason {
                        use config::ConfigRetrievalError;
                        eprint!("This path is used because it is {}", path.describe_for_choice_reasoning_suffix());
                        if let Err(err) = &config {
                            use std::borrow::Cow;
                            eprintln!(", but {}", match err {
                                ConfigRetrievalError::DeserializationFailure { .. } => Cow::Borrowed("it couldn't be successfully deserialized"),
                                ConfigRetrievalError::NotFound { .. } => Cow::Borrowed(if path.was_auto() { "it currently doesn't exist" } else { "it couldn't be found" }),
                                ConfigRetrievalError::PermissionDenied(_) => Cow::Borrowed("the required permissions to read it are not available"),
                                ConfigRetrievalError::UnknownFs { inner, .. } => Cow::Owned(format!("an unknown error occurred trying to read it ({inner})"))
                            });
                        } else {
                            eprintln!(".");
                        }
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
                            println!("Modifying configuration file @ {}", config.path.to_string_lossy());
                            config.edit_with_wizard().await;
                            config.save_to_disk().await;
                            println!("Successfully saved changes!");
                        },
                    }
                },
                #[cfg(feature = "discord")]
                ConfigurationAction::Discord { action } => {
                    use cli::DiscordConfigurationAction;
                    let mut config = get_config_or_error!();
                    if let Some(c) = config.backends.discord.as_mut() {
                        match action {
                            DiscordConfigurationAction::Enable => c.enabled = true,
                            DiscordConfigurationAction::Disable => c.enabled = false,
                        }
                    } else {
                        match action {
                            DiscordConfigurationAction::Enable => config::wizard::io::discord::prompt(&mut config.backends.discord, true),
                            DiscordConfigurationAction::Disable => {}
                        }
                    }
                    config.save_to_disk().await;
                }
            }
        }
    }

    ExitCode::SUCCESS
}

#[derive(Debug)]
struct PollingContext {
    terminating: Arc<AtomicBool>,
    backends: subscribers::Backends,
    pub last_track: Option<Arc<DispatchableTrack>>,
    pub listened: Arc<Mutex<Listened>>,
    artwork_manager: Arc<data_fetching::components::artwork::ArtworkManager>,
    
    #[cfg(feature = "musicdb")]
    musicdb: Arc<Option<musicdb::MusicDB>>,
    
    jxa: osa_apple_music::Session,
    app_open: bool,
    app_paused: Option<bool>,
    session: store::entities::Session,
}
impl PollingContext {
    async fn from_config(config: &config::Config, terminating: Arc<AtomicBool>) -> Self {
        let (backends, (artwork_manager, jxa, session, player_version)) = tokio::join!(
            subscribers::Backends::new(config),
            async {
                let ((pool, artwork_manager), (jxa, player_version)) = tokio::join!(
                    async {
                        let pool = store::DB_POOL.get().await.expect("failed to get database pool");
                        let artwork_manager = data_fetching::components::artwork::ArtworkManager::new(pool.clone(), &config.artwork_hosts).await;
                        (pool, artwork_manager)
                    },
                    async {
                        let jxa_socket = crate::util::APPLICATION_SUPPORT_FOLDER.join("osa-socket");
                        let mut jxa = osa_apple_music::Session::new(jxa_socket).await.expect("failed to create JXA session");
                        // TODO: Get the player version without JXA, so that the app doesn't need to be open.
                        let player_version = jxa.application().await.expect("failed to retrieve application data").map(|app| app.version).unwrap_or_else(|| "?".into());
                        (jxa, player_version)
                    }
                );

                store::migrations::migrate().await;

                let session = store::entities::Session::new(&pool, &player_version)
                    .await.unwrap_or_else(|err| ferror!("failed to create session in database: {}", err));

                (artwork_manager, jxa, session, player_version)
            }
        );

        Self {
            terminating,
            backends,
            last_track: None,
            listened: Arc::new(Mutex::new(Listened::new())),
            artwork_manager: Arc::new(artwork_manager),

            #[cfg(feature = "musicdb")]
            musicdb: Arc::new(if config.musicdb.enabled { Some(tracing::trace_span!("musicdb read").in_scope(|| {
                musicdb::MusicDB::read_path(config.musicdb.path.clone())
            })) } else { None }),
            
            jxa,
            app_open: player_version != "?",
            app_paused: None,
            session,
        }
    }

    async fn reload_from_config(&mut self, config: &config::Config) {
        self.backends = subscribers::Backends::new(config).await;
    }

    pub fn is_terminating(&self) -> bool {
        self.terminating.load(std::sync::atomic::Ordering::Relaxed)
    }
}

#[tracing::instrument(skip(context), level = "trace")]
async fn proc_once(context: Arc<Mutex<PollingContext>>) {
    let mut guard = context.lock().await;
    let context = guard.deref_mut();

    let app = match tracing::trace_span!("app status retrieval").in_scope(|| context.jxa.application()).await {
        Ok(Some(app)) => {
            context.app_open = true;
            Arc::new(app)
        },
        Ok(None) => {
            if !context.app_open { return; }
            tracing::debug!("app was closed; dispatching event");
            context.app_open = false;
            context.backends.dispatch_status(subscribers::DispatchedApplicationStatus::Closed).await;
            return;
        },
        Err(err) => {
            use osa_apple_music::error::SessionEvaluationError;
            match err {
                SessionEvaluationError::IoFailure(err) => tracing::error!(?err, "failed to retrieve application data"),
                SessionEvaluationError::SessionFailure(err) => tracing::error!(?err, "failed to extract application data"),
                SessionEvaluationError::ValueExtractionFailure { .. } => tracing::error!("failed to extract application data"),
                SessionEvaluationError::DeserializationFailure { issue, data, .. } => {
                    if !(issue.is_eof() && context.is_terminating()) {
                        tracing::error!(?issue, "failed to deserialize application data");
                        tracing::debug!("could not deserialize: {:?}", String::from_utf8_lossy(&data));
                    }
                },
                SessionEvaluationError::QueryFailure(err) => {
                    tracing::error!(?err, "failed to query application data");
                }
            }
            return;
        }
    };

    context.session.osa_fetches_player += 1;
    context.backends.dispatch_status(app.state.into()).await;
    use osa_apple_music::application::PlayerState;
    match app.state {
        PlayerState::FastForwarding | PlayerState::Rewinding => unimplemented!("unforeseen player state"),
        PlayerState::Stopped => {
            context.listened.lock().await.flush_current();
            
            if let Some(previous) = context.last_track.clone() {
                let listened = context.listened.clone();
                context.listened = Arc::new(Mutex::new(Listened::new()));
                context.last_track = None;
                context.backends.dispatch_track_ended(BackendContext {
                    listened,
                    track: previous,
                    app: app.clone(),
                    data: ().into(),
                    #[cfg(feature = "musicdb")]
                    musicdb: context.musicdb.clone()
                }).await;
            }
        }
        PlayerState::Paused => {},
        PlayerState::Playing => {
            let track = match context.jxa.now_playing().instrument(tracing::trace_span!("track retrieval")).await {
                Ok(Some(track)) => track,
                Ok(None) => return,
                Err(err) => {
                    use osa_apple_music::error::SessionEvaluationError;
                    match err {
                        SessionEvaluationError::IoFailure(err) => tracing::error!(?err, "failed to retrieve track data"),
                        SessionEvaluationError::SessionFailure(err) => tracing::error!(?err, "failed to retrieve track data"),
                        SessionEvaluationError::ValueExtractionFailure { .. } => tracing::error!("failed to extract track data"),
                        SessionEvaluationError::DeserializationFailure { issue, data, .. } => {
                            if !(issue.is_eof() && context.is_terminating()) {
                                tracing::error!(?issue, "failed to deserialize application data");
                                tracing::debug!("could not deserialize: {:?}", String::from_utf8_lossy(&data));
                            }
                        },
                        SessionEvaluationError::QueryFailure(err) => {
                            tracing::error!(?err, "failed to query application data");
                        }
                    }
                    return;
                }
            };

            context.session.osa_fetches_track += 1;

            // buffering / loading intermissions
            if track.kind.is_none() && (
                // TODO: What about other locales?
                //       I can't remember if there's a legit reason `track.kind` would be `None` that isn't loading, but maybe?
                track.name == "Connecting…" ||
                track.name.ends_with("Station")
            ) {
                return;
            }

            let track_playable_range = track.playable_range;
            let track = Arc::new(DispatchableTrack::from(track));

            let previous = context.last_track.as_ref().map(|v| &v.persistent_id);
            if previous != Some(&track.persistent_id) {
                tracing::trace!(?track, "new track");
                
                use data_fetching::AdditionalTrackData;
                let solicitation = context.backends.get_solicitations(subscription::Identity::TrackStarted).await;
                let additional_data_pending = AdditionalTrackData::from_solicitation(solicitation, track.as_ref(),
                    #[cfg(feature = "musicdb")]
                    context.musicdb.as_ref().as_ref(),
                    context.artwork_manager.clone()
                );

                let additional_data = if let Some(previous) = context.last_track.clone() {
                    let pending_dispatch = context.backends.dispatch_track_ended(BackendContext {
                        app: app.clone(),
                        track: previous,
                        listened: context.listened.clone(),
                        data: ().into(),
                        #[cfg(feature = "musicdb")]
                        musicdb: context.musicdb.clone()
                    }).instrument(tracing::trace_span!("song end dispatch"));

                    async move { 
                        // Run song-end dispatch concurrently while we fetch the additional data for the next
                        tokio::join!(
                            additional_data_pending,
                            pending_dispatch
                        )
                    }.await.0
                } else {
                    additional_data_pending.await
                };

                let track_start = app.position.or(track_playable_range.as_ref().map(|r| r.start)).unwrap_or(0.);
                let listened = Listened::new_with_current(track_start);
                let listened = Arc::new(Mutex::new(listened));
                context.listened = listened.clone();
                context.last_track = Some(track.clone());
                context.backends.dispatch_track_started(BackendContext {
                    app, listened, track,
                    data: Arc::new(additional_data),
                    #[cfg(feature = "musicdb")]
                    musicdb: context.musicdb.clone()
                }).await;
            } else if let Some(position) = app.position {
                let mut listened = context.listened.lock().await;
                match listened.current.as_ref() {
                    None => listened.set_new_current(position),
                    Some(current) => {
                        const MAX_DRIFT_BEFORE_REDISPATCH: f32 = 2.; // seconds;
                        let expected = current.get_expected_song_position();
                        if (expected - position).abs() >= MAX_DRIFT_BEFORE_REDISPATCH {
                            listened.flush_current();
                            listened.set_new_current(position);
                            drop(listened); // give up lock
                            context.backends.dispatch_current_progress(BackendContext {
                                track: track.clone(),
                                app: app.clone(),
                                data: ().into(),
                                listened: context.listened.clone(),
                                #[cfg(feature = "musicdb")]
                                musicdb: context.musicdb.clone()
                            }).await;
                        }
                    }
                }
            }
        }
    }
}
