
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
    /// Start the background service.
    Start,
    /// Stop the background service.
    Stop,
    /// Fully restart the background service.
    Restart,
    /// Reload the background service's configuration. (This may result in some funky behavior.)
    Reload
}

#[derive(Subcommand)]
pub enum ConfigurationAction {
    /// Run the configuration wizard. This will clear any existing settings.
    Wizard,

    /// Print the location of the configuration file that would be used in the current context, and why it's being used.
    #[clap(visible_alias("which"))]
    Where,

    /// Configure the Discord presence.
    Discord {
        #[command(subcommand)]
        action: DiscordConfigurationAction
    },
}

#[derive(Subcommand)]
pub enum DiscordConfigurationAction {
    /// Enable the Discord presence.
    Enable,

    /// Disable the Discord presence.
    Disable,

    // TODO: A way of changing the way the presence appears.
}
