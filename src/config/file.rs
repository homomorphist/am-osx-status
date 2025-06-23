use std::{borrow::Cow, path::Path};

use crate::util::HOME;

macro_rules! get_path_env_var { () => { "AM_OSX_STATUS_PATH" } }
pub static PATH_ENV_VAR: &str = get_path_env_var!();

const POST_HOME_DEFAULT_PATH: &str = "Library/Application Support/am-osx-status/config.toml";

/// How the user specified (or did not specify) the configuration file path.
#[derive(Clone, Debug)]
pub enum ConfigPathChoice<'a> {
    /// Explicitly provided by a flag in the CLI.
    /// This has the highest priority, and overrides the environmental variable and default path.
    Explicit(&'a std::path::Path),
    /// Inferred based on an environmental variable.
    /// This has the second-highest priority, overriding the default path but not one passed through a CLI flag.
    Environmental(std::ffi::OsString),
    /// Automatically determined path file based on the home directory, located in `~/Library/Application Support/am-osx-status/`.
    /// This is the default, hence the name.
    Automatic(std::path::PathBuf)
}
impl<'a> ConfigPathChoice<'a> {
    pub fn new(explicit: Option<&'a std::path::Path>) -> ConfigPathChoice<'a> {
        if let Some(explicit) = explicit {
            Self::Explicit(explicit)
        } else {
            std::env::var_os(PATH_ENV_VAR).map(Self::Environmental)
                .unwrap_or_else(Self::automatic)
        }
    }

    pub fn automatic() -> Self {
        Self::Automatic(HOME.join(POST_HOME_DEFAULT_PATH))
    }

    pub fn as_path(&self) -> &Path {
        match self {
            Self::Explicit(explicit) => explicit,
            Self::Environmental(environmental) => Path::new(environmental),
            Self::Automatic(automatic) => automatic
        }
    }

    pub fn to_string_lossy(&self) -> Cow<'_, str> {
        match self {
            Self::Explicit(str) => str.to_string_lossy(),
            Self::Automatic(buf) => buf.to_string_lossy(),
            Self::Environmental(os_string) => os_string.to_string_lossy()
        }
    }

    pub const fn describe_for_choice_reasoning_suffix(&self) -> &'static str {
        match self {
            Self::Explicit(_) => "explicitly provided",
            Self::Automatic(_) => "the application default",
            Self::Environmental(_) => concat!("sourced from the ", get_path_env_var!(), " environmental variable")
        }
    }

    pub const fn was_auto(&self) -> bool {
        matches!(self, Self::Automatic(..))
    }
}
impl AsRef<Path> for ConfigPathChoice<'_> {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}
impl core::default::Default for ConfigPathChoice<'_> {
    fn default() -> Self {
        Self::automatic()
    }
}

#[derive(thiserror::Error, Debug)]
enum ConfigFileAccessError {
    #[error("{0}")]
    PermissionDenied(std::io::Error),
    #[error("{0}")]
    DoesNotExist(std::io::Error),
    #[error("unknown io error: {0}")]
    Unknown(std::io::Error)
}
impl From<std::io::Error> for ConfigFileAccessError {
    fn from(error: std::io::Error) -> Self {
        use std::io::ErrorKind;
        match error.kind() {
            ErrorKind::PermissionDenied => Self::PermissionDenied(error),
            ErrorKind::NotFound => Self::DoesNotExist(error),
            _ => Self::Unknown(error)
        }
    }
}