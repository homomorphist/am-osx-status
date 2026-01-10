#![cfg(feature = "cli")]
pub use musicdb::*;

fn main() {
    #[cfg(feature = "cli-standalone")]
    tracing_subscriber::fmt::init();
    use clap::Parser;
    use cli::Arguments;
    Arguments::parse().command.handle();
}
