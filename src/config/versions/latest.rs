use serde::{Deserialize, Serialize};

use super::super::*;
pub use file::ConfigPathChoice;

#[derive(Serialize, Deserialize)]
pub struct Config {
    #[serde(skip)]
    pub path: ConfigPathChoice,
    #[serde(default)]
    pub backends: ConfigurableBackends,

    #[serde(
        default             = "crate::service::ipc::socket_path::clone_default",
        skip_serializing_if = "crate::service::ipc::socket_path::is_default",
    )]
    pub socket_path: std::path::PathBuf,

    #[serde(default)]
    pub artwork_hosts: crate::data_fetching::services::custom_artwork_host::HostConfigurations
}
impl Default for Config {
    fn default() -> Self {
        Self {
            path: Default::default(),
            backends: Default::default(),
            socket_path: crate::service::ipc::socket_path::clone_default(),
            artwork_hosts: Default::default(),
        }
    }
}
impl crate::config::LoadableConfig for Config {
    async fn edit_with_wizard(&mut self)  {
        #[cfg(feature = "discord")]
        wizard::io::discord::prompt(&mut self.backends.discord, false).await;
        #[cfg(feature = "lastfm")]
        wizard::io::lastfm::prompt(&mut self.backends.lastfm).await;
        #[cfg(feature = "listenbrainz")]
        wizard::io::listenbrainz::prompt(&mut self.backends.listenbrainz).await;
    }

    fn enrich(&mut self, path: ConfigPathChoice) {
        self.path = path;
    }

    fn get_path_choice(&self) -> &ConfigPathChoice {
        &self.path
    }
}
impl From<Config> for super::VersionedConfig {
    fn from(val: Config) -> Self {
        super::VersionedConfig::Latest(val)
    }
}

#[derive(Serialize, Deserialize)]
pub struct ConfigurableBackends {
    #[cfg(feature = "discord")]
    #[cfg_attr(feature = "discord", serde(default))]
    pub discord: Option<crate::subscribers::discord::Config>,
    #[cfg(feature = "lastfm")]
    #[cfg_attr(feature = "lastfm", serde(default))]
    pub lastfm: Option<crate::subscribers::lastfm::Config>,
    #[cfg(feature = "listenbrainz")]
    #[cfg_attr(feature = "listenbrainz", serde(default))]
    pub listenbrainz: Option<crate::subscribers::listenbrainz::Config>
}
#[allow(clippy::derivable_impls)]
impl Default for ConfigurableBackends {
    fn default() -> Self {
        Self {
            #[cfg(feature = "discord")]
            discord: Some(crate::subscribers::discord::Config::default()),
            #[cfg(feature = "lastfm")]
            lastfm: None,
            #[cfg(feature = "listenbrainz")]
            listenbrainz: None,
        }
    }
}
