#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(
    clippy::match_bool,
    clippy::wildcard_imports,
    clippy::too_many_lines,
    clippy::if_not_else,
    
    reason = "stylistic and explicitness preferences"
)]

extern crate alloc;
use alloc::sync::Arc;
use core::time::Duration;
use std::process::ExitCode;

use config::{ConfigPathChoice, ConfigRetrievalError};
use subscribers::{subscription, DispatchContext, DispatchableTrack};
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

type Terminating = Arc<std::sync::atomic::AtomicBool>;
type TerminationFuture = core::pin::Pin<Box<dyn core::future::Future<Output = tokio::signal::unix::SignalKind> + Send>>;

fn watch_for_termination() -> (
    Terminating,
    TerminationFuture,
) {
    use tokio::signal::unix::{SignalKind, signal};
    let flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut set = tokio::task::JoinSet::new();
    for kind in [
        SignalKind::quit(),
        SignalKind::hangup(),
        SignalKind::interrupt(),
        SignalKind::terminate(),
    ] {
        let mut signal = match signal(kind) {
            Ok(signal) => signal,
            Err(error) => { tracing::error!(?kind, ?error, "failed to register signal handler"); continue }
        };
        let flag = flag.clone();
        set.spawn(async move {
            signal.recv().await;
            flag.store(true, core::sync::atomic::Ordering::Relaxed);
            kind
        });
    }
    (
        flag,
        Box::pin(async move { set.join_next().await.unwrap().unwrap() })
    )
}

#[tokio::main(worker_threads = 2)]
async fn main() -> ExitCode {
    use cli::Command;

    let args = Box::leak(Box::new(<cli::Cli as clap::Parser>::parse()));
    let config = config::Config::get(args).await;
    let debugging = debugging::DebuggingSession::new(args);
    let (terminating, mut termination_signal) = watch_for_termination();

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

    match args.command {
        Command::Start { kill_existing } => {
            if let Some(pid) = ActiveProcessLockfile::get().await {
                if kill_existing {
                    unsafe { libc::kill(pid, libc::SIGTERM); }
                    tokio::time::sleep(Duration::from_millis(500)).await;
                } else {
                    eprintln!("Another instance of the program is already running! (pid {pid})");

                    if service::ServiceController::is_running().await {
                        eprintln!("You can turn off the service with `am-osx-status service stop`.");
                    }

                    return ExitCode::FAILURE;
                }
            }

            if let Err(error) = ActiveProcessLockfile::write().await {
                tracing::error!(?error, "failed to write active process lockfile");
            }

            let config = match get_config_or_path!() {
                Ok(config) => {
                    config.save_to_disk().await;
                    config
                },
                Err(path) => {
                    macro_rules! config_signal_cancellable {
                        ($expr: expr) => {
                            tokio::select! {
                                result = $expr => result,
                                signal = &mut termination_signal => {
                                    eprintln!("\nExiting; no configuration file will be made."); // '\n' to move past ^C printed by terminal and go to new line
                                    std::process::exit(128 + signal.as_raw_value());
                                }
                            }
                        };
                    }

                    let make = config_signal_cancellable!(config::wizard::io::prompt_bool(match path {
                        ConfigPathChoice::Automatic(..) => "No configuration has been set up! Would you like to use the wizard to build one?",
                        ConfigPathChoice::Explicit(..) => "No configuration exists at the provided file! Would you like to use the wizard to build it?",
                        ConfigPathChoice::Environmental(..) => "No configuration exists at the file specified in the environmental variable! Would you like to use the wizard to build it?",
                    }));

                    if make {
                        let config  = config_signal_cancellable!(config::Config::create_with_wizard(path));
                        config.save_to_disk().await;
                        println!("Configuration file has been saved.");
                        config
                    } else {
                        println!("Proceeding with a temporary default configuration.");
                        config::Config::default()
                    }

                }
            };

            let context = Arc::new(Mutex::new(PollingContext::from_config(&config, Arc::clone(&terminating)).await));
            let context_for_finalizer = Arc::clone(&context);

            let config = Arc::new(Mutex::new(config));

            let ipc_listener = if args.running_as_service {
                Some(service::ipc::listen(
                    context.clone(),
                    config.clone()
                ).await)
            } else { None };

            let main_loop = tokio::spawn(async move {
                tracing::info!("starting main loop");
                let mut interval = tokio::time::interval(POLL_INTERVAL);
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                while !terminating.load(core::sync::atomic::Ordering::Relaxed) {
                    proc_once(context.clone()).await;
                    tokio::time::sleep(POLL_INTERVAL).await;
                }
            });

            #[expect(clippy::significant_drop_tightening, reason = "lock is held for the remainder of the program lifetime during cleanup")]
            let finalizer = tokio::spawn(async move {
                let signal = termination_signal.await;
                tracing::debug!(?signal, "termination signal received; preparing for exit");

                tokio::select! {
                    result = main_loop => if let Err(error) = result { tracing::error!(?error, "main loop task panicked before termination could complete"); },
                    () = tokio::time::sleep(Duration::from_secs(5)) => { tracing::warn!("main loop did not quickly exit after termination signal; proceeding regardless"); }
                }

                let context = context_for_finalizer.lock().await;
                if let Some(ipc_listener) = ipc_listener { ipc_listener.abort(); }

                let db_pool = &store::DB_POOL.get().await.expect("failed to get database pool");
                let (cleared_lockfile, session_finished, ()) = tokio::join!(
                    ActiveProcessLockfile::clear(),
                    context.session.finish(db_pool),
                    context.backends.dispatch_handled::<subscription::type_identity::ImminentSubscriberTermination>(signal.into())
                );

                if let Err(error) = session_finished { tracing::error!(?error, "failed to finalize session in database"); }
                if let Err(error) = cleared_lockfile { tracing::error!(?error, "failed to clear active process lockfile"); }
                tracing::info!("exiting");
                drop(debugging.guards); // flush logs
            });

            finalizer.await.expect("finalizer task panicked");
        },
        Command::Service { ref action } => {
            use cli::ServiceAction;
            use service::ServiceController;

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
                #[cfg(debug_assertions)]
                ServiceAction::Reload => {
                    use service::ipc::{Packet, PacketConnection};
                    let path = get_config_or_error!().socket_path;
                    let mut connection = PacketConnection::from_path(path).await.unwrap();
                    connection.send(Packet::hello()).await.expect("failed to send hello packet");
                    connection.send(Packet::ReloadConfiguration).await.expect("failed to send reload packet");
                    println!("Reload command sent to service.");
                }
            }
        },
        Command::Configure { ref action } => {
            use cli::ConfigurationAction;

            tokio::spawn(async {
                termination_signal.await;
                std::process::exit(0);
            });

            match action {
                ConfigurationAction::Where { show_reason, escape} => {
                    use std::io::IsTerminal;

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

                    let show_reason = show_reason.unwrap_or_else(|| std::io::stdout().is_terminal());

                    println!("{path_str}");
                    if show_reason {
                        use config::ConfigRetrievalError;
                        eprint!("This path is used because it is {}", path.describe_for_choice_reasoning_suffix());
                        if let Err(err) = &config {
                            use alloc::borrow::Cow;
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
    terminating: Terminating,
    backends: subscribers::Backends,
    pub last_track: Option<subscribers::ActiveTrackContext>,
    artwork_manager: Arc<data_fetching::components::artwork::ArtworkManager>,
    
    #[cfg(feature = "musicdb")]
    musicdb: Arc<Option<musicdb::MusicDB>>,
    jxa: osa_apple_music::Session,
    player_open: bool,
    #[expect(dead_code, reason = "planned to be used in the future")]
    player_paused: Option<bool>,
    session: store::entities::Session,

    redispatch_start_requesters: Arc<Mutex<crate::subscribers::BackendIdentitySet>>, 
    redispatch_start_request_tx: tokio::sync::mpsc::Sender<crate::subscribers::BackendIdentity>,   
    #[expect(unused, reason = "attaching for drop")]
    redispatch_start_request_rx_processor: tokio::task::JoinHandle<()>,
}
impl PollingContext {
    async fn from_config(config: &config::Config, terminating: Terminating) -> Self {
        #[cfg(feature = "musicdb")]
        let musicdb: core::pin::Pin<Box<dyn Send + Future<Output = Result<Option<musicdb::MusicDB>, _>>>> = {
            let path = config.musicdb.path.clone();
            if config.musicdb.enabled { Box::pin(tokio::task::spawn_blocking(|| {
                let musicdb = tracing::trace_span!("musicdb read").in_scope(|| {
                    musicdb::MusicDB::read_path(path).expect("failed to read musicdb")
                });
                
                if let Some(installed) = util::get_installed_physical_memory() {
                    const MEMORY_WARNING_THRESHOLD: f64 = 100. / 8192.; // 100 MB on systems with 8 GB of RAM; approx 1.22% of RAM
                    #[expect(clippy::cast_precision_loss, reason = "acceptable loss of precision for this use case")]
                    let percentage = musicdb.get_raw().len() as f64 / installed as f64;
                    if percentage >= MEMORY_WARNING_THRESHOLD { tracing::warn!("musicdb handle is using {:.2}% of installed physical memory; disable it if this is a concern", percentage * 100.); }
                }

                Some(musicdb)
            })) } else { Box::pin(async { Ok(None) }) }
        };
        
        #[cfg(not(feature = "musicdb"))]
        let musicdb = Box::pin(async { Ok::<Option<()>, tokio::task::JoinError>(None) });

        let (redispatch_start_request_tx, mut redispatch_start_request_rx,) = tokio::sync::mpsc::channel(8);
        let redispatch_start_requesters = Arc::new(Mutex::new(crate::subscribers::BackendIdentitySet::empty()));
        let redispatch_start_request_rx_processor = {
            let redispatch_start_requesters = Arc::clone(&redispatch_start_requesters);
            tokio::spawn(async move {
                while let Some(identity) = redispatch_start_request_rx.recv().await {
                    tracing::debug!(?identity, "marking backend for a start event redispatch");
                    redispatch_start_requesters.lock().await.insert(identity);
                }
            })
        };

        let (backends, artwork_manager, migration_id, musicdb, (jxa, player_version)) = tokio::join!(
            subscribers::Backends::new(config, redispatch_start_request_tx.clone()),
            data_fetching::components::artwork::ArtworkManager::new(&config.artwork_hosts),
            store::migrations::migrate(),
            musicdb,
            async {
                let jxa_socket = crate::util::APPLICATION_SUPPORT_FOLDER.join("osa-socket");
                let mut jxa = osa_apple_music::Session::new(jxa_socket).await.expect("failed to create JXA session");
                // TODO: Get the player version without JXA, so that the player doesn't need to be open.
                let player_version = jxa.application().await.expect("failed to retrieve application data").map_or_else(|| "?".into(), |app| app.version);
                (jxa, player_version)
            }
        );

        let session = store::entities::Session::new(&player_version, migration_id)
            .await.unwrap_or_else(|err| ferror!("failed to create session in database: {}", err));

        #[cfg_attr(not(feature = "musicdb"), expect(unused_variables, reason = "unused when disabled"))]
        let musicdb = match musicdb {
            Ok(musicdb) => Arc::new(musicdb),
            Err(error) => {
                tracing::error!(?error, "failed to open musicdb");
                Arc::new(None)
            }
        };

        Self {
            terminating,
            backends,
            last_track: None,
            artwork_manager: Arc::new(artwork_manager),
            #[cfg(feature = "musicdb")]
            musicdb,
            jxa,
            player_open: player_version != "?",
            player_paused: None,
            session,

            redispatch_start_requesters,
            redispatch_start_request_tx,
            redispatch_start_request_rx_processor
        }
    }

    async fn reload_from_config(&mut self, config: &config::Config) {
        self.backends = subscribers::Backends::new(config, self.redispatch_start_request_tx.clone()).await;
    }

    pub fn is_terminating(&self) -> bool {
        self.terminating.load(core::sync::atomic::Ordering::Relaxed)
    }

    async fn dispatch_track_end(&self, player: Arc<osa_apple_music::application::ApplicationData>, track: subscribers::ActiveTrackContext) {
        track.listened.lock().await.flush_current();
        self.backends.dispatch_handled::<subscription::type_identity::TrackEnded>(DispatchContext {
            #[cfg(feature = "musicdb")]
            musicdb: self.musicdb.clone(),
            data: Arc::new(subscribers::ActivePlayerContext {
                data: player.clone(),
                track,
            }),
        }).await;
    }
}


#[expect(clippy::significant_drop_tightening, reason = "concurrent execution of this function is undesirable")]
#[tracing::instrument(skip(context), level = "trace")]
async fn proc_once(context: Arc<Mutex<PollingContext>>) {
    use subscription::type_identity as E;

    let mut guard = context.lock().await;
    let context = &mut *guard;

    let player = match tracing::trace_span!("player status retrieval").in_scope(|| context.jxa.application()).await {
        Ok(Some(player)) => {
            context.player_open = true;
            Arc::new(player)
        },
        Ok(None) => {
            if !context.player_open { return; }
            tracing::debug!("player was closed; dispatching event");
            context.player_open = false;
            context.backends.dispatch_handled::<E::PlayerStatusUpdate>(subscribers::DispatchedPlayerStatus::Closed).await;
            return;
        },
        Err(err) => {
            use osa_apple_music::error::SessionEvaluationError;
            match err {
                SessionEvaluationError::IoFailure(err) => tracing::error!(?err, "failed to retrieve player data"),
                SessionEvaluationError::SessionFailure(err) => tracing::error!(?err, "failed to extract player data"),
                SessionEvaluationError::ValueExtractionFailure { .. } => tracing::error!("failed to extract player data"),
                SessionEvaluationError::DeserializationFailure { issue, data, .. } => {
                    if !(issue.is_eof() && context.is_terminating()) {
                        tracing::error!(?issue, "failed to deserialize player data");
                        tracing::debug!("could not deserialize: {:?}", String::from_utf8_lossy(&data));
                    }
                },
                SessionEvaluationError::QueryFailure(err) => {
                    tracing::error!(?err, "failed to query player data");
                }
            }
            return;
        }
    };

    context.session.osa_fetches_player += 1;
    context.backends.dispatch_handled::<E::PlayerStatusUpdate>(player.state.into()).await;

    use osa_apple_music::application::PlayerState;
    match player.state {
        PlayerState::Stopped => {
            if let Some(last_track) = context.last_track.take() {
                context.dispatch_track_end(player.clone(), last_track).await;
            }
        }
        PlayerState::Paused => {},
        state @ (PlayerState::Playing | PlayerState::FastForwarding | PlayerState::Rewinding) => {
            if state != PlayerState::Playing {
                // TODO: Figure out how we want to handle this. https://github.com/homomorphist/am-osx-status/issues/61
                tracing::warn!(?state, "unsupported player state encountered; treating as normal continuous playback. behavior might be funky");
            }

            let track = match context.jxa.current_track().instrument(tracing::trace_span!("track retrieval")).await {
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

            // Don't process temporary tracks that are used to signify the buffering of the next track.
            if track.album.track_count == 0 && track.playable_range.is_some_and(|d| d.end == 0.) {
                return;
            }

            let track_playable_range = track.playable_range;
            let track = Arc::new(DispatchableTrack::from_track(track, #[cfg(feature = "musicdb")] context.musicdb.as_ref().as_ref()).await);

            let previous = context.last_track.as_ref().map(|v| &v.persistent_id);
            if previous != Some(&track.persistent_id) {
                tracing::debug!(?track, "new track");

                let solicitation = context.backends.get_solicitations(subscription::Identity::TrackStarted).await;
                let additional_data_pending = data_fetching::AdditionalTrackData::from_solicitation(solicitation, track.as_ref(),
                    #[cfg(feature = "musicdb")]
                    context.musicdb.as_ref().as_ref(),
                    context.artwork_manager.clone()
                );

                let additional_data = if let Some(previous) = context.last_track.take() {
                    let pending_dispatch = context.dispatch_track_end(player.clone(), previous);

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

                let track_start = player.position.or_else(|| track_playable_range.as_ref().map(|r| r.start)).unwrap_or(0.);
                let listened = Listened::new_with_current(track_start);
                let listened = Arc::new(Mutex::new(listened));

                let track = subscribers::ActiveTrackContext {
                    data: track.clone(),
                    listened: listened.clone(),
                };

                context.last_track = Some(track.clone());
                context.backends.dispatch_handled::<E::TrackStarted>(DispatchContext {
                    #[cfg(feature = "musicdb")]
                    musicdb: context.musicdb.clone(),
                    data: Arc::new((subscribers::ActivePlayerContext { data: player.clone(), track }, additional_data)),
                }).await;
            } else if let Some(position) = player.position {
                let track = context.last_track.clone().unwrap();

                {
                    use subscribers::subscription::type_identity::TrackStarted;
                    use subscribers::BackendIdentitySet;

                    let mut requesting_redispatch = context.redispatch_start_requesters.lock().await;
                    if !requesting_redispatch.is_empty() { let list = *requesting_redispatch; tracing::debug!(?list, "performing start redispatch"); }
                    let backends = context.backends.get_many(*requesting_redispatch);

                    let solicitation = context.backends.get_solicitations_from(backends.clone(), subscription::Identity::TrackStarted).await; // why clone needed :(
                    let additional_data = data_fetching::AdditionalTrackData::from_solicitation(solicitation, track.as_ref(),
                        #[cfg(feature = "musicdb")]
                        context.musicdb.as_ref().as_ref(),
                        context.artwork_manager.clone()
                    ).await;
                    
                    context.backends.dispatch_to::<TrackStarted>(backends, DispatchContext {
                        #[cfg(feature = "musicdb")]
                        musicdb: context.musicdb.clone(),
                        data: Arc::new((subscribers::ActivePlayerContext {
                            data: player.clone(),
                            track: track.clone(),
                        }, additional_data)),
                    }).await;

                    *requesting_redispatch = BackendIdentitySet::default();
                }


                let mut listened = track.listened.lock().await;
                match listened.current.as_ref() {
                    None => listened.set_new_current(position),
                    Some(current) => {
                        const MAX_DRIFT_BEFORE_REDISPATCH: f32 = 2.; // seconds;
                        let expected = current.get_expected_song_position();
                        if (expected - position).abs() >= MAX_DRIFT_BEFORE_REDISPATCH {
                            listened.flush_current();
                            listened.set_new_current(position);
                            drop(listened); // give up lock
                            context.backends.dispatch::<E::ProgressJolt>(DispatchContext {
                                #[cfg(feature = "musicdb")]
                                musicdb: context.musicdb.clone(),
                                data: Arc::new(subscribers::ActivePlayerContext {
                                    data: player.clone(),
                                    track
                                }),
                            }).await;
                        }
                    }
                }
            }
        }
    }
}
