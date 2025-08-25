#![allow(unused)]
use std::{ops::DerefMut, process::ExitCode, sync::{atomic::AtomicBool, Arc}, time::Duration};
use config::{ConfigPathChoice, ConfigRetrievalError};
use subscribers::{subscription, BackendContext, DispatchableTrack};
use tokio::sync::Mutex;
use tracing::Instrument;
use listened::Listened;
use util::ferror;

use crate::service::{ServiceRestartFailure, ServiceStopFailure};
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
mod error_layer;

const POLL_INTERVAL: Duration = Duration::from_millis(500);

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
            let config = match get_config_or_path!() {
                Ok(config) => config,
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
                drop(listener); // remove listener socket
                drop(debugging.guards); // flush logs
                let db_pool = &store::DB_POOL.get().await.expect("failed to get database pool");
                let session = &mut session_closer_ctx.lock().await.session;
                session.ended_at = Some(chrono::Utc::now().into());
                session.finish(db_pool).await.expect("failed to finish session in database");
                std::process::exit(1);
            });


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

            let controller = service::ServiceController::new();

            #[derive(thiserror::Error, Debug)]
            enum ServiceFailure {
                #[error("could not start service: {0}")]
                Start(#[from] ServiceStartFailure),
                #[error("could not stop service: {0}")]
                Stop(#[from] ServiceStopFailure),
                #[error("could not restart service: {0}")]
                Restart(#[from] ServiceRestartFailure),
                #[error("could not reload service: {0}")]
                Reload(#[from] ServiceIpcError)
            }

            #[derive(thiserror::Error, Debug)]
            enum ServiceIpcError {
                #[error("failed to dispatch IPC packet ({0})")]
                Dispatch(#[from] std::io::Error),
                #[error("failed to establish IPC connection ({0})")]
                Connection(std::io::Error),
            }

            if let Err(error) = match action {
                ServiceAction::Start => controller.start(get_config_os_string!(), false).map_err(ServiceFailure::from),
                ServiceAction::Stop => controller.stop().map_err(ServiceFailure::from),
                ServiceAction::Restart => controller.restart(get_config_os_string!()).map_err(ServiceFailure::from),
                ServiceAction::Reload => async {
                    use ipc::{Packet, PacketConnection};
                    let path = get_config_or_error!().socket_path;
                    let mut connection = PacketConnection::from_path(path).await.map_err(ServiceIpcError::Connection)?;
                    connection.send(Packet::hello()).await?;
                    connection.send(Packet::ReloadConfiguration).await?;
                    Ok::<_, ServiceIpcError>(())
                }.await.map_err(ServiceFailure::from)
            } {
                tracing::error!(%error);
                ferror!("{error}");
            }
        },
        Command::Configure { ref action } => {
            tokio::spawn(async {
                pending_term.await;
                std::process::exit(1);
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
                            DiscordConfigurationAction::Enable => config::wizard::io::discord::prompt(&mut config.backends.discord, true).await,
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
    paused: Option<bool>,
    session: store::entities::Session,
}
impl PollingContext {
    async fn from_config(config: &config::Config, terminating: Arc<AtomicBool>) -> Self {
        let (backends, (artwork_manager, jxa, session)) = tokio::join!(
            subscribers::Backends::new(config),
            async {
                let ((pool, artwork_manager), (jxa, player_version)) = tokio::join!(
                    async {
                        let pool = store::DB_POOL.get().await.expect("failed to get database pool");
                        let artwork_manager = data_fetching::components::artwork::ArtworkManager::new(pool.clone(), &config.artwork_hosts).await;
                        (pool, artwork_manager)
                    },
                    async {
                        let jxa_socket = crate::util::HOME.join("Library/Application Support/am-osx-status/osa-socket");
                        let mut jxa = osa_apple_music::Session::new(jxa_socket).await.expect("failed to create JXA session");
                        // TODO: Get the player version without JXA, so that the app doesn't need to be open.
                        let player_version = jxa.application().await.expect("failed to retrieve application data").map(|app| app.version).unwrap_or_else(|| "?".into());
                        (jxa, player_version)
                    }
                );

                let session = store::entities::Session::new(&pool, &player_version)
                    .await.unwrap_or_else(|err| ferror!("failed to create session in database: {}", err));

                (artwork_manager, jxa, session)
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
                musicdb::MusicDB::read_path(config.musicdb.path.clone().unwrap_or(musicdb::MusicDB::default_path()))
            })) } else { None }),

            paused: None,
            jxa,
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
        Ok(Some(app)) => Arc::new(app),
        Ok(None) => return,
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
