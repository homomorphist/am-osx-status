use serde::{Deserialize, Serialize};

mod file;
pub mod versions;
pub mod wizard;

pub use versions::latest::*;
pub use file::ConfigPathChoice;

#[derive(Debug, thiserror::Error)]
pub enum ConfigRetrievalError {
    #[error("unexpected file system error: {inner}")]
    UnknownFs { #[source] inner: std::io::Error, path: ConfigPathChoice },
    #[error("deserialization failure: {inner}")]
    DeserializationFailure { #[source] inner: toml::de::Error, path: ConfigPathChoice },
    #[error("file did not exist")]
    NotFound(ConfigPathChoice),
    #[error("permission denied reading path {}", .0.to_string_lossy())]
    PermissionDenied(ConfigPathChoice)
}
impl ConfigRetrievalError {
    pub const fn path(&self) -> &ConfigPathChoice {
        match self {
            Self::UnknownFs { path, .. } |
            Self::DeserializationFailure { path, .. } |
            Self::NotFound(path) |
            Self::PermissionDenied(path) => path,
        }
    }
}

pub trait LoadableConfig where Self: Sized + for <'de> Deserialize<'de> + Serialize + Into<versions::VersionedConfig> {
    async fn get(args: &'static crate::cli::Cli) -> Result<Self, ConfigRetrievalError> {
        let path_override = args.config_file_path.as_deref();
        let path = ConfigPathChoice::new(path_override);
        Self::from_path(path).await
    }

    async fn from_path(path: ConfigPathChoice) -> Result<Self, ConfigRetrievalError> {
        match tokio::fs::read(&path).await {
            Err(error) => {
                use tokio::io::ErrorKind;
                match error.kind() {
                    ErrorKind::PermissionDenied => Err(ConfigRetrievalError::PermissionDenied(path)),
                    ErrorKind::NotFound => Err(ConfigRetrievalError::NotFound(path)),
                    _ => Err(ConfigRetrievalError::UnknownFs { inner: error, path })
                }
            },
            Ok(data) => {
                let data = String::from_utf8_lossy(&data[..]);
                match toml::from_str::<Self>(&data) {
                    Err(inner) => Err(ConfigRetrievalError::DeserializationFailure { inner, path }),
                    Ok(mut config) => {
                        config.enrich(path);
                        Ok(config)
                    }
                }
            }
       }
    }

    /// Adorn the configuration struct with meta- information that is not stored on disk,
    /// such as the path to the configuration file and how it was specified.
    fn enrich(&mut self, path: ConfigPathChoice);

    /// Get the path to the configuration file.
    fn get_path_choice(&self) -> &ConfigPathChoice;

    fn serialize(&self) -> String {
        toml::ser::to_string(self).expect("could not serialize constructed configuration")
    }

    async fn edit_with_wizard(&mut self);

    /// NOTE: Will not write to the provided path unless [`Self::save_to_disk`] is called.
    async fn create_with_wizard(path: ConfigPathChoice) -> Self where Self: Default {
        let mut config: Self = Default::default();
        config.edit_with_wizard().await;
        config.enrich(path);
        config
    }
    
    async fn reload_from_disk(&mut self) -> Result<(), ConfigRetrievalError> {
        let new = Self::from_path(self.get_path_choice().clone()).await?;
        *self = new;
        Ok(())
    }

    async fn save_to_disk(&self) {
        let path = self.get_path_choice().as_path();
        tokio::fs::create_dir_all(path.parent().expect("cannot write to root...?")).await.expect("could not create configuration directory");
        tokio::fs::write(&path, LoadableConfig::serialize(self).as_bytes()).await.expect("could not write configuration");
    }

    #[expect(unused, reason = "versioned configurations are not fully implemented")]
    fn upgrade(self) -> versions::VersionedConfig {
        self.into().upgrade()
    }
}
