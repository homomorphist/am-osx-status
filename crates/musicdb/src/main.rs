#![cfg(feature = "cli")]
pub use musicdb::*;

fn main() {
    use clap::Parser;
    use cli::Arguments;
    Arguments::parse().command.handle();
}
