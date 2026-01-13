#![cfg(feature = "cli")]
pub use musicdb::*;

fn main() {
    #[cfg(feature = "cli-standalone")]
    musicdb::setup_tracing_subscriber();
    use clap::Parser;
    use cli::Arguments;
    Arguments::parse().command.handle();
}
