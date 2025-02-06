use std::{cell::{Cell, RefCell}, os::fd::{AsRawFd, IntoRawFd}};

use kqueue::FilterFlag;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};

use crate::{util::{ferror, HOME}};

pub mod wizard;
mod file;
pub use file::ConfigPathChoice;


enum PathChoice<'a> {
    Explicit(&'a str),
    Environmental
}

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
    pub backends: ConfigurableBackends,


    #[serde(skip)]
    file_descriptor: Mutex<Option<std::os::fd::OwnedFd>>,
    #[serde(skip)]
    file_watcher: Option<kqueue::Watcher>,
    #[serde(default = "ret_true")]
    pub watch_config_file: bool,
    #[serde(skip_serializing_if = "crate::service::ipc::socket_path::is_default", default = "crate::service::ipc::socket_path::clone_default")]
    pub ipc_socket_path: std::path::PathBuf,
}
impl Default for Config<'_> {
    fn default() -> Self {
        Self {
            ipc_socket_path: crate::service::ipc::socket_path::DEFAULT.clone(),
            backends: ConfigurableBackends::default(),
            path: ConfigPathChoice::default(),
            watch_config_file: false,
            file_descriptor: None.into(),
            file_watcher: None,
        }
    }
}
impl<'a> Config<'a> {
    pub async fn get(args: &'a crate::cli::Cli) -> Result<Self, ConfigRetrievalError<'a>> {
        let path_override = args.config_file_path.as_deref();
        let path = ConfigPathChoice::new(path_override);
    
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
                Ok(toml::from_str(&data).map_err(|err| ConfigRetrievalError::DeserializationFailure { inner: err, path })?)
            }
       }
    }

    pub async fn get_fd(&self) -> Option<std::os::fd::RawFd> {
        if self.file_descriptor.lock().await.is_none() {
            self.file_descriptor.lock().await.insert(match tokio::fs::File::options().open(&self.path).await {
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => return None,
                Err(err) => { println!("error: {:?}", err); return None }
                Ok(file) => file.into_std().await.into()
            });
        }

        self.file_descriptor.lock().await
            .as_ref()
            .map(|v| v.as_raw_fd())
    }

    pub async fn update_on_file_change(&mut self, enable: bool) {
        if self.watch_config_file == enable { return }
        self.watch_config_file = enable;
        if enable {
            let fd = self.get_fd().await.expect("cannot get fd");
            let watcher = self.file_watcher.as_mut().unwrap(); // TODO: make if not present
            watcher.add_fd(fd, kqueue::EventFilter::EVFILT_WRITE, FilterFlag::empty()).expect("aaa");
            watcher.watch().expect("aaaa")
        } else {
            drop(self.file_watcher.take())
        }
    }

    pub async fn edit_with_wizard(&mut self)  {
        self.backends.discord = wizard::io::prompt_bool("Enable Discord Rich Presence?");
        self.backends.lastfm =  wizard::io::prompt_lastfm().await;
        self.backends.listenbrainz = wizard::io::prompt_listenbrainz().await;
    }

    /// NOTE: Will not write to the provided path unless [`Self::save_to_disk`] is called.
    pub async fn create_with_wizard(path: ConfigPathChoice<'a>) -> Self {
        let mut config: Self = Default::default();
        config.edit_with_wizard();
        config
    }

    pub fn serialize(&self) -> String {
        toml::ser::to_string(self).expect("could not serialize constructed configuration")
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
    pub discord: bool,
    #[cfg(feature = "lastfm")]
    pub lastfm: Option<crate::status_backend::lastfm::Config>,
    #[cfg(feature = "listenbrainz")]
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
