#[allow(unused)]
pub struct DebuggingGuards {
    appender: Option<tracing_appender::non_blocking::WorkerGuard>
}

pub struct DebuggingSession {
    pub guards: DebuggingGuards
}

impl DebuggingSession {
    pub fn new(args: &crate::cli::Cli) -> Self {
        use tracing_subscriber::prelude::*;

        let mut layers = Vec::with_capacity(4);
        let mut appender_guard = None;

        layers.push(tracing_subscriber::fmt::layer().boxed());

        if cfg!(debug_assertions) && !args.running_as_service {
            #[cfg(feature = "tokio_console")]
            layers.push({
                console_subscriber::ConsoleLayer::builder()
                    .spawn()
                    .boxed()
            });
        }

        if let Ok(created) = Self::make_logging_dir() {
            tracing::debug!(%created, "logging directory ready");

            let appender = tracing_appender::rolling::Builder::default()
                .filename_suffix("log")
                .rotation(tracing_appender::rolling::Rotation::DAILY)
                .max_log_files(3)
                .build(crate::util::HOME.join("Library/Logs/am-osx-status"))
                .expect("failed to create rolling file appender");

            let (non_blocking, guard) = tracing_appender::non_blocking(appender);

            layers.push(tracing_subscriber::fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false)
                .boxed()
            );

            appender_guard = Some(guard);
        } else {
            eprintln!("WARNING: failed to create logging directory, file logging disabled");
        }

        layers.push(tracing_oslog::OsLogger::new(crate::util::REVERSE_DNS_IDENTIFIER, "default").boxed());

        tracing_subscriber::registry()
            .with(Self::get_filter(args))
            .with(layers)
            .init();

        std::panic::set_hook(Box::new(panic_hook));
    
        let guards = DebuggingGuards {
            appender: appender_guard
        };
    
        Self {
            guards
        }
    }

    /// Create the logging directory if it doesn't already exist. Returns `Ok(true)` if it was created, `Ok(false)` if it already existed.
    fn make_logging_dir() -> Result<bool, std::io::Error> {
        match std::fs::create_dir(crate::util::HOME.join("Library/Logs/am-osx-status")) {
            Ok(()) => Ok(true),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
            Err(err) => Err(err)
        }
    }

    /// Get the filter for log output. The `AMXS_LOG` environmental variable takes priority over CLI arguments.
    fn get_filter(args: &crate::cli::Cli) -> tracing_subscriber::EnvFilter {
        use tracing_subscriber::EnvFilter;

        const ENV: &str = "AMXS_LOG";
        if std::env::var_os(ENV).is_some() {
            if args.verbose.is_present() {
                eprintln!("WARNING: Provided verbosity arguments were ignored as environmental variable {ENV} is set");
            }
            EnvFilter::try_from_env(ENV).expect("bad log filter")
        } else {
            let level = if args.verbose.is_present() {
                args.verbose.tracing_level().map_or("none", |level| level.as_str())
            } else {
                tracing::Level::INFO.as_str()
            };

            EnvFilter::new(level)
        }
    }
}
impl core::default::Default for DebuggingSession {
    fn default() -> Self {
        Self {
            guards: DebuggingGuards {
                appender: None
            }
        }
    }
}

// thanks a lot https://github.com/rust-lang/rust/issues/67939
// theoretically this could break across std versions but blehhh,, no way that'll happen, right?
fn extract_thread_id(id: std::thread::ThreadId) -> core::num::NonZero<u64> {
    unsafe { 
        core::mem::transmute::<
            std::thread::ThreadId,
            core::num::NonZero<u64>
        >(id)
    }
}

fn panic_hook(info: &std::panic::PanicHookInfo) {
    use std::backtrace::*;
    use core::panic::Location;

    let backtrace = Backtrace::capture();
    let location = info.location().map(Location::to_string);
    let message = info.payload_as_str();
    let thread = std::thread::current();
    let thread_id = extract_thread_id(thread.id());

    tracing::error!(
        location = location,
        backtrace = tracing::field::display(match backtrace.status() {
            BacktraceStatus::Captured => format!("r#\"\n{backtrace}\"#"),
            BacktraceStatus::Disabled => "disabled (run with RUST_BACKTRACE=1)".to_string(),
            BacktraceStatus::Unsupported => "unsupported".to_string(),
            opt => format!("unknown (unrecognized status {opt:?})"),
        }),
        "{} (T{}) panicked at {}",
        thread.name().map_or_else(|| "unnamed thread".to_owned(), |name| format!("thread '{name}'")),
        thread_id,
        message.unwrap_or("<no message>")
    );

    if thread_id.get() == 1 {
        std::process::exit(1)
    }
}
