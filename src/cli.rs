
use clap_verbosity_flag::Verbosity;
use clap::{Parser, Subcommand};

/// Apple Music status utility for MacOS.
#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// The path to the configuration file to load.
    #[arg(short, long = "config", value_name = "PATH", global = true)]
    pub config_file_path: Option<std::path::PathBuf>,

    #[arg(hide = true, long = "ran-as-service", default_value = "false")]
    pub running_as_service: bool,

    #[command(flatten)]
    pub verbose: Verbosity,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Start, stop, or reload the background service.
    Service {
        #[command(subcommand)]
        action: ServiceAction
    },
    /// Begin watching Apple Music and log information.
    Start,
    /// Configure the application.
    #[clap(visible_alias("config"))]
    Configure {
        #[command(subcommand)]
        action: ConfigurationAction
    }
}


#[derive(Subcommand)]
pub enum ServiceAction {
    /// Start the background service. It will then automatically start on every login.
    Start,
    /// Stop the background service. It will start again on the next login, or when started again manually.
    Stop,
    /// Log information about the status of the background service.
    Status,
    /// Uninstall the background service.
    Remove,
    /// Fully restart the background service.
    Restart,
    #[cfg_attr(debug_assertions, doc = "Reload the background service's configuration. (This may result in some funky behavior.)")]
    #[cfg(debug_assertions)]
    Reload
}

#[derive(Subcommand)]
pub enum ConfigurationAction {
    /// Run the configuration wizard. This will clear any existing settings.
    Wizard,

    /// Print the location of the configuration file that would be used in the current context.
    #[clap(visible_alias("which"))]
    Where {
        /// Explain why the configuration file is being used, and if there were any issues trying to read it.
        /// This will be enabled by default if standard output is detected as a terminal.
        #[arg(short = 'r', long = "reason", aliases = ["why", "explain"])]
        show_reason: Option<bool>,
        /// Escape special characters (such as spaces) in the path.
        #[arg(short, long, default_value = "false")]
        escape: bool,
    },

    /// Configure the Discord presence.
    #[cfg(feature = "discord")]
    Discord {
        #[command(subcommand)]
        action: DiscordConfigurationAction
    },
}

#[cfg(feature = "discord")]
#[derive(Subcommand)]
pub enum DiscordConfigurationAction {
    /// Enable the Discord presence.
    Enable,

    /// Disable the Discord presence.
    Disable,

    // TODO: A way of changing the way the presence appears.
}
