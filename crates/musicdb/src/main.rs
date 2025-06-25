#![cfg(feature = "cli")]

static IS_PIPING_OUTPUT: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
    use std::io::IsTerminal;
    !std::io::stdout().is_terminal()
});

fn stripping_prefix<'a, 'b>(prefixes: impl AsRef<[&'a str]>, value: &'b str) -> Option<&'b str> {
    let prefixes = prefixes.as_ref();
    for prefix in prefixes {
        if let Some(rest) = value.strip_prefix(prefix) {
            return Some(rest)
        }
    }
    None
}
fn parse_ambiguous_id(id: &str) -> Result<u64, core::num::ParseIntError> {
    if let Some(hex) = stripping_prefix(["0x", "0X"], id) {
        return u64::from_str_radix(hex, 16)
    }
    if let Some(dec) = stripping_prefix(["0d", "0D"], id) {
        return dec.parse::<u64>()
    }
    if id.chars().any(|c| !c.is_ascii_digit()) {
        return u64::from_str_radix(id, 16)
    }
    if id.len() > 16 {
        // couldn't be hex
        return id.parse::<u64>()
    }

    panic!("Base of ID {id} is unknown; please specify explicitly with `0x` (hex) or `0d` (dec) prefix.");
}

fn read_ids(passed: Vec<String>) -> Vec<u64> {
    let mut out = Vec::with_capacity(passed.len()); // prob gonna be larger; can be passed as csv in each one
    for given in passed {
        for str in given.split(",").map(str::trim) {
            out.push(parse_ambiguous_id(str).expect("bad id")) // todo: better error handling
        }
    }
    out
}

fn main() {
    use std::io::Write;
    use args::*;
    use musicdb::MusicDB;

    match Arguments::parse().command {
        Command::Decrypt { path, output } => {
            let raw = MusicDB::extract_raw(path.unwrap_or_else(MusicDB::default_path)).expect("failed to extract raw data");
            let is_stdout = output.as_ref() == Some(&Destination::Stdout);
            let mut writer = std::io::BufWriter::new(output.unwrap_or_default().into_writer());

            if let Err(error) = writer.write_all(&raw) {
                eprintln!("Write error: {error:?}");
            } else if !is_stdout {
                println!("Done!");
            }
        }


        Command::Export { path, output , ids } => {
            let mut musicdb = path.map(MusicDB::read_path).unwrap_or_default();
            let musicdb = musicdb.get_view_mut();

            if let Some(filter) = ids {
                let filter = read_ids(filter);

                macro_rules! filter_map {
                    ($v: expr, $filter: ident) => {
                        {
                            $v.0.retain(|id, _| $filter.contains(&id.get_raw()));
                        }
                    }
                }

                macro_rules! filter_set {
                    ($v: expr, $filter: ident) => {
                        {
                            $v.0.retain(|v| $filter.contains(&::musicdb::id::persistent::Possessor::get_persistent_id(v).get_raw()));
                        }
                    }
                }

                musicdb.library.0.clear(); // fuk u
                filter_map!(musicdb.artists, filter);
                filter_map!(musicdb.albums, filter);
                filter_map!(musicdb.tracks, filter);
                filter_set!(musicdb.collections, filter);
                if let Some(accounts) = &mut musicdb.accounts {
                    filter_set!(accounts, filter)
                }
            }

            let exported = format!("{musicdb:#?}").replace("    ", "\t");
            let is_stdout = output.as_ref() == Some(&Destination::Stdout);
            let mut writer = std::io::BufWriter::new(output.unwrap_or_default().into_writer());

            if let Err(error) = writer.write_all(exported.as_bytes()) {
                eprintln!("Write error: {error:?}");
            } else if !is_stdout {
                println!("Done!");
            }
        }
    }
}


mod args {
    use std::path::PathBuf;

    pub use clap::{Parser, Subcommand};

    /// `.musicdb` file exporting utility.
    #[derive(Parser)]
    #[command(version, about, long_about = None)]
    pub struct Arguments {
        #[command(subcommand)]
        pub command: Command,
    }

    #[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub enum Destination {
        #[default]
        Stdout,
        Path(std::path::PathBuf),
    }
    impl<'a> From<&'a str> for Destination {
        fn from(str: &'a str) -> Self {
            if str == "-" {
                Destination::Stdout
            } else {
                Destination::Path(std::path::PathBuf::from(str))
            }
        }
    }
    impl Destination {
        pub fn into_writer(self) -> Box<dyn std::io::Write> {
            match self {
                Destination::Path(path) => Box::new(std::fs::File::create(path).expect("failed to create file")),
                Destination::Stdout => Box::new(std::io::stdout().lock()),
            }
        }
    }

    #[derive(Subcommand)]
    pub enum Command {
        /// Export a decrypted (but not decoded) `.musicdb` file.
        Decrypt {
            /// The path to the `Library.musicdb` file to export. Defaults to the one of the current user.
            #[arg(short, long, value_name = "PATH")]
            path: Option<PathBuf>,

            /// The destination path ('-' for stdout) to write to.
            /// Must be explicitly provided, unless being piped (in which case stdout is chosen).
            #[arg(short, long, value_name = "TARGET", alias = "out", required = !*crate::IS_PIPING_OUTPUT)]
            output: Option<Destination>,
        },

        /// Export a decoded `.musicdb` file.
        Export {
            /// The path to the `Library.musicdb` file to export. Defaults to the one of the current user.
            #[arg(short, long, value_name = "PATH")]
            path: Option<PathBuf>,

            /// The destination path ('-' for stdout) to write to.
            /// Must be explicitly provided, unless being piped (in which case stdout is chosen).
            #[arg(short, long, value_name = "TARGET", alias = "out", required = !*crate::IS_PIPING_OUTPUT)]
            output: Option<Destination>,

            /// Entity persistent-IDs to filter the output with. Base-10 or base-16 (case-insensitive),
            /// coma-separated or passed over multiple arguments.
            #[arg(short, long, value_name = "ID", alias = "ids")]
            ids: Option<Vec<String>>,
        },
    }
}
