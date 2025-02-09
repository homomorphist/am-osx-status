#[allow(unused)]
struct DebuggingGuards {
    chrome_tracing: Option<tracing_chrome::FlushGuard>
}

pub struct DebuggingSession {
    #[allow(unused)]
    guards: DebuggingGuards
}

impl DebuggingSession {
    pub fn new(args: &crate::cli::Cli) -> Self {
        use tracing_subscriber::prelude::*;

        let mut layers = Vec::with_capacity(3);
        let mut chrome_guard = None;

        if cfg!(debug_assertions) {
            layers.push(console_subscriber::spawn().boxed());
            layers.push(tracing_subscriber::fmt::layer().boxed());
            if !args.running_as_service {
                let (chrome_layer, chrome_guard_unraised) = tracing_chrome::ChromeLayerBuilder::new().build();
                chrome_guard = Some(chrome_guard_unraised);
                layers.push(chrome_layer.boxed());
            }
        }

        tracing_subscriber::registry()
            .with(Self::get_filter(args))
            .with(layers)
            .init();
    
        let guards = DebuggingGuards {
            chrome_tracing: chrome_guard
        };
    
        Self {
            guards
        }
    }


    /// Get the filter for log output. The `AMXS_LOG`` environmental variable takes priority over CLI arguments.
    fn get_filter(args: &crate::cli::Cli) -> tracing_subscriber::EnvFilter {
        use tracing_subscriber::EnvFilter;

        const ENV: &str = "AMXS_LOG";
        if std::env::var_os(ENV).is_some() {
            if args.verbose.is_present() {
                eprintln!("WARNING: Provided verbosity arguments were ignored as environmental variable {ENV} is set");
            }
            EnvFilter::try_from_env(ENV).expect("bad log filter")
        } else if let Some(verbosity) = args.verbose.tracing_level() {
            EnvFilter::new(verbosity.as_str())
        } else {
            EnvFilter::new("none")
        }
    }
}
impl core::default::Default for DebuggingSession {
    fn default() -> Self {
        Self {
            guards: DebuggingGuards {
                chrome_tracing: None
            }
        }
    }
}