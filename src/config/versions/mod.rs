use serde::{Deserialize, Serialize};

use crate::config::{ConfigPathChoice, ConfigRetrievalError};

pub mod latest;

#[derive(Deserialize)]
#[serde(tag = "config_schema")]
pub enum VersionedConfig {
    /// Unstable: latest version of the config schema.
    #[serde(rename = "latest")]
    Latest(latest::Config),
}
impl VersionedConfig {
    pub fn into_latest(self) -> latest::Config {
        let mut config = self;
        loop {
            match config {
                Self::Latest(config) => return config,
                outdated => config = outdated.upgrade(),
            }
        }
    }

    pub fn upgrade(self) -> Self {
        match self {
            Self::Latest(config) => Self::Latest(config),
        }
    }
}
