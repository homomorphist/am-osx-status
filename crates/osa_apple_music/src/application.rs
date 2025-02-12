#![allow(unused)]
use serde::Deserialize;

#[derive(Deserialize, Debug, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum PlayerState {
    Stopped,
    Playing,
    Paused,
    #[serde(rename = "fast forwarding")]
    FastForwarding,
    Rewinding,
}

/// How the application is configured to shuffle tracks.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum ShuffleMode {
    Songs,
    Albums,
    Groupings
}


#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum RepeatMode {
    /// There is no repeat target.
    Off,
    /// The current track is repeated.
    One,
    /// All tracks in the current playlist are repeated.
    All,
}

/// The state of the Apple Music application.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationData {
    /// The current state of the player.
    #[serde(rename = "playerState")]
    pub state: PlayerState,

    /// The version of the application.
    pub version: String,

    /// Whether the application is muted.
    pub mute: bool,

    #[serde(rename = "shuffleEnabled")]
    pub shuffling: bool,

    /// The configured shuffle mode of the application.
    #[serde(rename = "shuffleMode")]
    pub shuffle: Option<ShuffleMode>,

    /// The configured repeat mode of the application.
    #[serde(rename = "songRepeat")]
    pub repeat: RepeatMode,

    /// The configured volume of the application; an integer from 0 to 100, inclusive on both ends.
    #[serde(rename = "soundVolume")]
    pub volume: u8,

    /// The position of the current track in seconds.
    pub position: Option<u32>,
}
impl ApplicationData {
    pub(crate) fn fix(mut self) -> Self {
        if !self.shuffling {
            self.shuffle = None;
        }
        self
    }

    /// Fetches and returns the application state.
    /// If you find yourself doing this repeatedly, consider using [`Session`](crate::Session) instead.
    pub async fn fetch() -> Result<Self, crate::error::SingleEvaluationError> {
        osascript::run("JSON.stringify(Application(\"Music\").properties())", osascript::Language::JavaScript)
            .await
            .map_err(crate::error::SingleEvaluationError::IoError)
            .and_then(|output| { Ok(serde_json::from_str(&output.stdout()).map(ApplicationData::fix)?) })
    }
}
