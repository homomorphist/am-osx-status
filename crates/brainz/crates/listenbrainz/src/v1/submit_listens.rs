#[derive(Debug, Clone, Copy)]
pub enum ListenType {
    Single,
    PlayingNow,
    Import
}
impl ListenType {
    pub const fn to_str(&self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::PlayingNow => "playing_now",
            Self::Import => "import"
        }
    }
}
impl core::fmt::Display for ListenType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.to_str())
    }
}
impl serde::Serialize for ListenType {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.to_str())
    }
}

#[derive(serde::Serialize, Debug)]
pub struct BasicTrackMetadata<'a> {
    #[serde(rename = "artist_name")] pub artist: &'a str,
    #[serde(rename = "track_name")] pub track: &'a str,
    #[serde(rename = "release_name", skip_serializing_if = "Option::is_none")] pub release: Option<&'a str>
}

#[derive(serde::Serialize, Debug)]
pub(crate) struct ListeningPayloadTrackMetadata<'a> {
    #[serde(flatten)]
    pub basic: BasicTrackMetadata<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_info: Option<additional_info::Raw<'a>>,
}

#[derive(serde::Serialize, Debug)]
pub(crate) struct RawBody<'a> {
    pub listen_type: ListenType,
    pub payload: &'a [ListeningPayload<'a>]
}
impl RawBody<'_> {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("cannot encode")
    }
}


#[derive(serde::Serialize, Debug)]
pub(crate) struct ListeningPayload<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listened_at: Option<u32>,
    #[serde(rename = "track_metadata")]
    pub metadata: ListeningPayloadTrackMetadata<'a>
}

pub mod additional_info {
    use musicbrainz::{*, request_client::ProgramInfo};

    pub struct MediaPlayer<'a> {
        pub name: &'a str,
        pub version: Option<&'a str>
    }

    
    pub enum MusicService<'a> {
        Domain(&'a str),
        DomainIndeterminate { name: &'a str }
    }
    
    #[derive(Debug, Default)]
    pub struct BrainzIds {
        pub artists: Option<Vec<Id<entities::Artist>>>,
        pub release_group: Option<Id<entities::ReleaseGroup>>,
        pub release: Option<Id<entities::Release>>,
        pub recording: Option<Id<entities::Recording>>,
        pub track: Option<Id<entities::Track>>,
        pub works: Option<Vec<Id<entities::Work>>>
    }
    
    #[derive(Default)]
    pub struct AdditionalInfo<'a> {
        pub ids: BrainzIds,
        pub track_number: Option<u32>,
        pub isrc: Option<&'a str>,
        pub tags: Vec<Tag<'a>>,
        pub music_service: Option<MusicService<'a>>,
        pub submission_client: Option<&'a ProgramInfo<maybe_owned_string::MaybeOwnedStringDeserializeToOwned<'a>>>,
        pub media_player: Option<MediaPlayer<'a>>,
        pub origin_url: Option<&'a str>,
        pub duration: Option<core::time::Duration>
    }
    impl<'a> AdditionalInfo<'a> {
        pub(crate) fn into_raw(self) -> Raw<'a> {
            Raw {
                recording_mbid: self.ids.recording.map(Id::contextless),
                release_group_mbid: self.ids.release_group.map(Id::contextless),
                release_mbid: self.ids.release.map(Id::contextless),
                track_mbid: self.ids.track.map(Id::contextless),
                work_mbids: self.ids.works.map(|vec| vec.into_iter().map(Id::contextless).collect()),
                artist_mbids: self.ids.artists.map(|vec| vec.into_iter().map(Id::contextless).collect()),
                tracknumber: self.track_number.map(|n| n.to_string()),
                isrc: self.isrc,
                tags: self.tags.iter().map(|tag| unimplemented!()).collect(),
                media_player: self.media_player.as_ref().map(|player| player.name),
                media_player_version: self.media_player.as_ref().and_then(|player| player.version),
                submission_client: self.submission_client.as_ref().map(|player| player.name.as_ref()),
                submission_client_version: self.submission_client.as_ref().and_then(|player| player.version.as_ref().map(|version| version.as_ref())),
                music_service: self.music_service.as_ref().and_then(|ms| match ms { MusicService::Domain(d) => Some(*d), _ => None }),
                music_service_name: self.music_service.and_then(|ms| match ms { MusicService::DomainIndeterminate { name } => Some(name), _ => None }),
                origin_url: self.origin_url,
                duration_ms: self.duration.map(|duration| duration.as_millis() as u64),
                duration: None,
            }
        }
    }

    use shared::HyphenatedUuidString;

    /// - <https://listenbrainz.readthedocs.io/en/latest/users/json.html#id1>
    #[derive(serde::Serialize, Debug)]
    pub(crate) struct Raw<'a> {
        #[serde(skip_serializing_if = "Option::is_none")] pub artist_mbids: Option<Vec<HyphenatedUuidString>>,
        #[serde(skip_serializing_if = "Option::is_none")] pub release_group_mbid: Option<HyphenatedUuidString>,
        #[serde(skip_serializing_if = "Option::is_none")] pub release_mbid: Option<HyphenatedUuidString>,
        #[serde(skip_serializing_if = "Option::is_none")] pub recording_mbid: Option<HyphenatedUuidString>,
        #[serde(skip_serializing_if = "Option::is_none")] pub track_mbid: Option<HyphenatedUuidString>,
        #[serde(skip_serializing_if = "Option::is_none")] pub work_mbids: Option<Vec<HyphenatedUuidString>>,
        #[serde(skip_serializing_if = "Option::is_none")] pub tracknumber: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")] pub isrc: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")] pub tags: Option<Vec<&'a str>>,
        #[serde(skip_serializing_if = "Option::is_none")] pub media_player: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")] pub media_player_version: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")] pub submission_client: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")] pub submission_client_version: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")] pub music_service: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")] pub music_service_name: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")] pub origin_url: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")] pub duration_ms: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")] pub duration: Option<u64>,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ListenSubmissionError {
    #[error(transparent)]
    HistoricDateError(#[from] super::error::ListenDateTooHistoric),
    #[error("network failure: {0}")]
    NetworkFailure(#[from] reqwest::Error),
    #[error("ratelimited")]
    Ratelimited,
    #[error(transparent)]
    InvalidToken(#[from] super::error::InvalidTokenError),
    #[error("error {0}: {1}")]
    Other(reqwest::StatusCode, String)
}

#[derive(Debug, thiserror::Error)]
pub enum CurrentlyPlayingSubmissionError {
    #[error("network failure: {0}")]
    NetworkFailure(#[from] reqwest::Error),
    #[error("ratelimited")]
    Ratelimited,
    #[error(transparent)]
    InvalidToken(#[from] super::error::InvalidTokenError),
    #[error("error {0}: {1}")]
    Other(reqwest::StatusCode, String)
}

