#[cfg(feature = "cli")]
fn main() {
    use args::*;
    use musicdb::*;

    match Arguments::parse().command {
        Command::Export { path, output } => {
            let musicdb = path.map(MusicDB::read_path).unwrap_or_default();
            let exported = format!("{musicdb:#?}");
            if let Some(output) = output {
                if let Err(error) = std::fs::write(output, exported) {
                    eprintln!("Error writing to file: {error:?}");
                } else {
                    println!("Done!");
                }
            } else {
                println!("{}", exported);
            }
        }
    }
}

#[cfg(feature = "cli")]
mod args {
    pub use clap::{Parser, Subcommand};

    /// `.musicdb` file exporting utility.
    #[derive(Parser)]
    #[command(bin_name = "musicdb", version, about, long_about = None)]
    pub struct Arguments {
        #[command(subcommand)]
        pub command: Command,
    }

    #[derive(Subcommand)]
    pub enum Command {
        /// Export a `.musicdb` file.
        Export {
            /// The path to the `Library.musicdb` file to export. Defaults to the one of the current user.
            #[arg(short, long, value_name = "PATH")]
            path: Option<std::path::PathBuf>,

            /// The file to write to. If not specified, the output will be printed to stdout.
            #[arg(short, long, value_name = "PATH", alias = "out")]
            output: Option<std::path::PathBuf>,
        },
    }
}
