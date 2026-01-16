use std::path::PathBuf;
use clap::{Parser, Subcommand};

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

pub fn parse_ambiguous_id(id: &str) -> Result<u64, core::num::ParseIntError> {
    if let Some(hex) = stripping_prefix(["0x", "0X"], id) {
        return u64::from_str_radix(hex, 16)
    }
    if let Some(dec) = stripping_prefix(["0d", "0D"], id) {
        if let Some(signed) = dec.strip_suffix("i") {
            let signed = signed.parse::<i64>()?;
            return Ok(signed.cast_unsigned())
        }
        return dec.parse::<u64>()
    }
    if let Some(dec) = id.strip_suffix("i") {
        let signed = dec.parse::<i64>()?;
        return Ok(signed.cast_unsigned())
    }
    if id.chars().any(|c| !c.is_ascii_digit()) {
        return u64::from_str_radix(id, 16)
    }
    if id.len() > 16 {
        // couldn't be hex 
        return id.parse::<u64>()
    }

    eprintln!("Base of ID {id} is unknown; please specify explicitly with `0x` (hex) or `0d` (dec) prefix.");
    std::process::exit(1)
}

pub fn parse_ambiguous_ids(passed: Vec<String>) -> Vec<u64> {
    let mut out = Vec::with_capacity(passed.len()); // prob gonna be larger; can be passed as csv in each one
    for given in passed {
        for str in given.split(",").map(str::trim) {
            out.push(parse_ambiguous_id(str).expect("bad id")) // todo: better error handling
        }
    }
    out
}

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
    /// Unpack the raw data from a `.musicdb` file.
    Unpack {
        /// The path to the `Library.musicdb` file to export. Defaults to the one of the current user.
        #[arg(short, long, value_name = "PATH")]
        path: Option<PathBuf>,

        /// The destination path ('-' for stdout) to write to.
        /// Must be explicitly provided, unless being piped (in which case stdout is chosen).
        #[arg(short, long, value_name = "TARGET", alias = "out", required = !*IS_PIPING_OUTPUT)]
        output: Option<Destination>,
    },

    /// Export a fully parsed `.musicdb` file.
    Export {
        /// The path to the `Library.musicdb` file to export. Defaults to the one of the current user.
        #[arg(short, long, value_name = "PATH")]
        path: Option<PathBuf>,

        /// The destination path ('-' for stdout) to write to.
        /// Must be explicitly provided, unless being piped (in which case stdout is chosen).
        #[arg(short, long, value_name = "TARGET", alias = "out", required = !*IS_PIPING_OUTPUT)]
        output: Option<Destination>,

        /// Entity persistent-IDs to filter the output with. Base-10 or base-16 (case-insensitive),
        /// comma-separated or passed over multiple arguments.
        #[arg(short, long, value_name = "ID", alias = "ids")]
        ids: Option<Vec<String>>,
    },

    /// Print the compression ratio(s) of the `.musicdb` file(s), recursively searching directories.
    #[cfg(debug_assertions)]
    #[clap(alias = "ratio")]
    Ratios {
        #[arg(value_name = "PATH")]
        paths: Option<Vec<std::path::PathBuf>>,
    }
}

impl Command {
    pub fn handle(self) {
        use std::io::Write;
        use crate::MusicDB;

        match self {
            Command::Unpack { path, output } => {
                let unpacked = MusicDB::unpack(path.unwrap_or_else(MusicDB::default_path)).expect("failed to extract raw data");
                let is_stdout = output.as_ref() == Some(&Destination::Stdout);
                let mut writer = std::io::BufWriter::new(output.unwrap_or_default().into_writer());

                if let Err(error) = writer.write_all(&unpacked) {
                    eprintln!("Write error: {error:?}");
                } else if !is_stdout {
                    println!("Done!");
                }
            }

            Command::Export { path, output , ids } => {
                let mut musicdb = MusicDB::read_path(path.unwrap_or_else(MusicDB::default_path)).expect("failed to read musicdb");
                let musicdb = musicdb.get_view_mut();

                if let Some(filter) = ids {
                    let filter = parse_ambiguous_ids(filter);

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
                                $v.0.retain(|v| $filter.contains(&$crate::id::persistent::Possessor::get_persistent_id(v).get_raw()));
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
        
            #[cfg(debug_assertions)]
            Command::Ratios { paths } => {
                use crate::MusicDB;
                use std::fs;

                let paths = paths.unwrap_or_else(|| vec![MusicDB::default_path()]);
                let mut paths = std::collections::VecDeque::from(paths);
                let mut ratios = Vec::with_capacity(paths.len());
                while let Some(path) = paths.pop_front() {
                    if path.is_dir() {
                        for entry in fs::read_dir(&path).expect("failed to read dir") {
                            let entry = entry.expect("failed to read dir entry");
                            paths.push_back(entry.path());
                        }
                    } else if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("musicdb") {
                        let unpacked = match MusicDB::unpack(&path) {
                            Ok(unpacked) => unpacked,
                            Err(error) => {
                                eprintln!("Failed to decode {}: {}", path.display(), error);
                                continue;
                            }
                        };

                        let size = fs::metadata(&path).expect("failed to get metadata").len();
                        let ratio = unpacked.len() as f64 / size as f64;
                        println!("{}: {:.2} ({} -> {})", path.display(), ratio, size, unpacked.len());
                        ratios.push(ratio);
                    }
                }

                if ratios.len() > 1 {
                    println!("---");
                    let avg = ratios.iter().sum::<f64>() / ratios.len() as f64;
                    let (min, max) = ratios.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), &r| { (min.min(r), max.max(r)) });
                    let stddev = (ratios.iter().map(|r| (r - avg).powi(2)).sum::<f64>() / ratios.len() as f64).sqrt();
                    println!("Average: {:.2}, Min: {:.2}, Max: {:.2}, StdDev: {:.2}", avg, min, max, stddev);
                }
            }
        }
    }
}
