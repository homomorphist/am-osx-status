use std::os::fd::AsRawFd;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

pub mod wizard;
mod file;
pub use file::ConfigPathChoice;


#[derive(Debug, thiserror::Error)]
pub enum ConfigRetrievalError<'a> {
    #[error("unexpected file system error: {inner}")]
    UnknownFs { #[source] inner: std::io::Error, path: ConfigPathChoice<'a> },
    #[error("deserialization failure: {inner}")]
    DeserializationFailure { #[source] inner: toml::de::Error, path: ConfigPathChoice<'a> },
    #[error("file did not exist")]
    NotFound(ConfigPathChoice<'a>),
    #[error("permission denied reading path {}", .0.to_string_lossy())]
    PermissionDenied(ConfigPathChoice<'a>)
}
impl<'a> ConfigRetrievalError<'a> {
    pub fn path(&self) -> &ConfigPathChoice<'a> {
        match self {
            Self::UnknownFs { path, .. } => path,
            Self::DeserializationFailure { path, .. } => path,
            Self::NotFound(path) => path,
            Self::PermissionDenied(path) => path,
        }
    }
}

fn ret_true() -> bool {
    true
}


#[derive(Serialize, Deserialize)]
pub struct Config<'a> {
    #[serde(skip)]
    pub path: ConfigPathChoice<'a>,
    #[serde(default)]
    pub backends: ConfigurableBackends,

    #[serde(
        default             = "crate::service::ipc::socket_path::clone_default",
        skip_serializing_if = "crate::service::ipc::socket_path::is_default",
    )]
    pub socket_path: std::path::PathBuf
}
impl Default for Config<'_> {
    fn default() -> Self {
        Self {
            path: Default::default(),
            backends: Default::default(),
            socket_path: crate::service::ipc::socket_path::clone_default(),
        }
    }
}
impl<'a> Config<'a> {
    pub async fn get(args: &'a crate::cli::Cli) -> Result<Self, ConfigRetrievalError<'a>> {
        let path_override = args.config_file_path.as_deref();
        let path = ConfigPathChoice::new(path_override);
        Self::from_path(path).await
    }

    pub async fn from_path(path: ConfigPathChoice<'a>) -> Result<Self, ConfigRetrievalError<'a>> {
        match std::fs::read(&path) {
            Err(error) => {
                use std::io::ErrorKind;
                match error.kind() {
                    ErrorKind::PermissionDenied => Err(ConfigRetrievalError::PermissionDenied(path)),
                    ErrorKind::NotFound => Err(ConfigRetrievalError::NotFound(path)),
                    _ => Err(ConfigRetrievalError::UnknownFs { inner: error, path })
                }
            },
            Ok(data) => {
                let data = String::from_utf8_lossy(&data[..]);
                match toml::from_str::<Config>(&data) {
                    Err(inner) => Err(ConfigRetrievalError::DeserializationFailure { inner, path }),
                    Ok(mut config) => {
                        config.path = path;
                        Ok(config)
                    }
                }
            }
       }
    }

    pub async fn edit_with_wizard(&mut self)  {
        self.backends.discord = wizard::io::prompt_bool("Enable Discord Rich Presence?");
        wizard::io::prompt_lastfm(&mut self.backends.lastfm).await;
        wizard::io::prompt_listenbrainz(&mut self.backends.listenbrainz).await;
    }

    /// NOTE: Will not write to the provided path unless [`Self::save_to_disk`] is called.
    pub async fn create_with_wizard(path: ConfigPathChoice<'a>) -> Self {
        let mut config: Self = Default::default();
        config.edit_with_wizard().await;
        config.path = path;
        config
    }

    pub fn serialize(&self) -> String {
        toml::ser::to_string(self).expect("could not serialize constructed configuration")
    }

    pub async fn reload_from_disk(&mut self) -> Result<(), ConfigRetrievalError<'a>> {
        let new = Self::from_path(self.path.clone()).await?;;
        *self = new;
        Ok(())
    }
    pub async fn save_to_disk(&self) {
        let path = self.path.as_path();
        tokio::fs::create_dir_all(path.parent().expect("cannot write to root...?")).await.expect("could not create configuration directory");
        tokio::fs::write(&path, self.serialize().as_bytes()).await.expect("could not write configuration");
    }
}


#[derive(Serialize, Deserialize)]
pub struct ConfigurableBackends {
    #[cfg(feature = "discord")]
    #[cfg_attr(feature = "discord", serde(default = "ret_true"))]
    pub discord: bool,
    #[cfg(feature = "lastfm")]
    #[cfg_attr(feature = "lastfm", serde(default))]
    pub lastfm: Option<crate::status_backend::lastfm::Config>,
    #[cfg(feature = "listenbrainz")]
    #[cfg_attr(feature = "listenbrainz", serde(default))]
    pub listenbrainz: Option<crate::status_backend::listenbrainz::Config>
}
impl Default for ConfigurableBackends {
    fn default() -> Self {
        Self {
            #[cfg(feature = "discord")]
            discord: true,
            #[cfg(feature = "lastfm")]
            lastfm: None,
            #[cfg(feature = "listenbrainz")]
            listenbrainz: None,
        }
    }
}
