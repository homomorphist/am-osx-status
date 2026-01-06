use std::path::Path;
use alloc::borrow::Cow;

use crate::util::APPLICATION_SUPPORT_FOLDER;

macro_rules! get_path_env_var { () => { "AM_OSX_STATUS_PATH" } }
pub static PATH_ENV_VAR: &str = get_path_env_var!();

/// How the user specified (or did not specify) the configuration file path.
#[derive(Clone, Debug)]
pub enum ConfigPathChoice {
    /// Explicitly provided by a flag in the CLI.
    /// This has the highest priority, and overrides the environmental variable and default path.
    Explicit(&'static std::path::Path),
    /// Inferred based on an environmental variable.
    /// This has the second-highest priority, overriding the default path but not one passed through a CLI flag.
    Environmental(std::ffi::OsString),
    /// Automatically determined path file based on the home directory, located in `~/Library/Application Support/am-osx-status/`.
    /// This is the default, hence the name.
    Automatic(std::path::PathBuf)
}
impl ConfigPathChoice {
    #[expect(clippy::option_if_let_else, reason = "suggestion looks ugly")]
    pub fn new(explicit: Option<&'static std::path::Path>) -> Self {
        if let Some(explicit) = explicit {
            Self::Explicit(explicit)
        } else {
            std::env::var_os(PATH_ENV_VAR).map_or_else(Self::automatic, Self::Environmental)
        }
    }

    pub fn automatic() -> Self {
        Self::Automatic(APPLICATION_SUPPORT_FOLDER.join("config.toml"))
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
impl AsRef<Path> for ConfigPathChoice {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}
impl core::default::Default for ConfigPathChoice {
    fn default() -> Self {
        Self::automatic()
    }
}
