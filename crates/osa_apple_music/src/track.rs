#![allow(unused)]

type Time = chrono::DateTime<chrono::Utc>;

use std::{default, num::NonZeroU8};

use serde_with::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

macro_rules! non_zero_conv {
    ($(($ty: ty, $ident: ident) $(,)?)*) => {
        $(
            serde_with::serde_conv!(
                $ident,
                Option<core::num::NonZero<$ty>>,
                |v: &Option<core::num::NonZero<$ty>>,| v.map(|v| v.get()).unwrap_or(0),
                |value: $ty| -> Result<_, std::convert::Infallible> {
                    Ok(core::num::NonZero::<$ty>::new(value))
                }
            );
        )*
    }
}


non_zero_conv!(
    (u8, u8_ZeroAsNone),
    (u16, u16_ZeroAsNone)
);

serde_with::serde_conv!(
    optional_f32_duration,
    Option<core::time::Duration>,
    |v: &Option<core::time::Duration>| v.map(|v| v.as_secs_f32()),
    |value: f32| -> Result<_, std::convert::Infallible> {
        if value == 0.0 {
            Ok(None)
        } else {
            Ok(Some(core::time::Duration::from_secs_f32(value)))
        }
    }
);

rating::def_rating!({}, Rating);
impl<T> PartialEq<T> for Rating where T: AsRef<Rating> {
    fn eq(&self, other: &T) -> bool {
        self == other.as_ref()
    }
}


pub(crate) mod rating {
    use super::*;

    #[macro_export]
    macro_rules! def_rating {
        ({ $(#[$meta: meta])* }, $ident: ident) =>  {
            #[derive(Debug, Deserialize, Serialize, PartialEq)]
            $(#[$meta])*
            pub enum $ident {
                User(u8),
                Computed(u8)
            }
        };
        ({ $(#[$meta: meta])* }, $ident: ident, equivalent => $into: ident) => {
            def_rating!({ $(#[$meta])* }, $ident);
            impl From<$ident> for $into {
                fn from(value: $ident) -> $into {
                    unsafe { core::mem::transmute(value) }
                }
            }
            impl From<$into> for $ident {
                fn from(value: $into) -> $ident {
                    unsafe { core::mem::transmute(value) }
                }
            }
            impl AsRef<$into> for $ident {
                fn as_ref(&self) -> &$into {
                    unsafe { core::mem::transmute(self) }
                }
            }
        }
    }

    pub use def_rating;

    def_rating!({
        /// The rating of a track's album.
        #[serde(tag = "albumRatingKind", content = "albumRating", rename_all = "lowercase")]
    }, ForTrackAlbum, equivalent => Rating);
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")] // with space delimiters
pub enum TrackCloudStatus {
    /// The cloud status of this track is unknown / ineligible.
    Unknown,
    Purchased,
    /// This track exists locally, but was linked up to a version available for streaming.
    Matched,
    /// A local file that has been uploaded to iCloud and can be streamed on other devices authorized with the same Apple ID.
    Uploaded,
    Ineligible,
    Removed,
    Error,
    Duplicate,
    /// This track is available as a result of a subscription to Apple Music.
    Subscription,
    Prerelease,
    /// The track is no longer available for streaming.
    /// This could occur, for example, if a track were removed because of licensing issues.
    #[serde(rename = "no longer available")]
    NoLongerAvailable,
    #[serde(rename = "not uploaded")]
    NotUploaded
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TrackDownloader {
    /// The Apple ID of the person who downloaded this track.
    #[serde(rename = "downloaderAppleID")]
    apple_id: String,
    /// The name of the person who downloaded this track.
    #[serde(rename = "downloaderName")]
    name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TrackPurchaser {
    /// The Apple ID of the person who purchased this track.
    #[serde(rename = "purchaserAppleID")]
    apple_id: String,
    /// The name (or email) of the person who purchased this track.
    #[serde(rename = "purchaserName")]
    name: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(feature = "sqlx", sqlx(rename_all = "lowercase"))]
pub enum MediaKind {
    Song,
    #[cfg_attr(feature = "sqlx", sqlx(rename = "music video"))]
    #[serde(rename = "music video")]
    MusicVideo,
    Unknown
}


#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeDetails {
    /// The episode ID of the track.
    #[serde(rename = "episodeID")]
    pub episode_id: String,

    /// The episode number of the track.
    pub episode_number: u16,
}


#[derive(Debug, Deserialize, Serialize)]
pub struct PlayedInfo {
    /// Number of times this track has been played.
    #[serde(rename = "playedCount")]
    times: u32,

    /// The date and time this track was last played.
    #[serde(rename = "playedDate")]
    last: Option<Time>,

    /// Whether this track has never been played before.
    #[serde(rename = "unplayed")]
    never: bool
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SkippedInfo {
    /// Number of times this track has been skipped.
    #[serde(rename = "skippedCount")]
    times: u32,

    /// The date and time this track was last skipped.
    #[serde(rename = "skippedDate")]
    last: Option<Time>,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct MovementInfo {
    /// The name of the movement.
    #[serde(rename = "movement")]
    name: String,

    /// The index of this movement in the work.
    #[serde(rename = "movementNumber")]
    index: u16,
}

serde_with::serde_conv!(
    MovementInfoOrNone,
    Option<MovementInfo>,
    |v: &(Option<MovementInfo>)| v.clone().unwrap_or_default(),
    |value: MovementInfo| -> Result<_, std::convert::Infallible> {
       if value.name.is_empty() {
            Ok(None)
        } else {
            Ok(Some(value))
        }
    }
);


#[serde_as]
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SortingOverrides {
    /// Override string to use for the track when sorting by album
    #[serde_as(as = "NoneAsEmptyString")]
    #[serde(rename = "sortAlbum")]
    pub album: Option<String>,

    /// Override string to use for the track when sorting by artist
    #[serde_as(as = "NoneAsEmptyString")]
    #[serde(rename = "sortArtist")]
    pub artist: Option<String>,

    /// Override string to use for the track when sorting by album artist
    #[serde_as(as = "NoneAsEmptyString")]
    #[serde(rename = "sortAlbumArtist")]
    pub album_artist: Option<String>,

    /// Override string to use for the track when sorting by name
    #[serde_as(as = "NoneAsEmptyString")]
    #[serde(rename = "sortName")]
    pub name: Option<String>,

    /// Override string to use for the track when sorting by composer
    #[serde_as(as = "NoneAsEmptyString")]
    #[serde(rename = "sortComposer")]
    pub composer: Option<String>,

    /// Override string to use for the track when sorting by show name
    #[serde_as(as = "NoneAsEmptyString")]
    #[serde(rename = "sortShow", default)]
    pub show: Option<String>,
}


#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
pub struct PlayableRange {
    /// The start of the playable region, in seconds. Defaults to zero.
    pub start: f32,

    /// The end of the playable region, in seconds. Defaults to the duration of the song.
    #[serde(rename = "finish")]
    pub end: f32,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum EqualizerPreset {
    Acoustic,
    Classical,
    Dance,
    Deep,
    Electronic,
    Flat,
    #[serde(rename = "Hip-Hop")]
    HipHop,
    #[serde(rename = "Increase Bass")]
    IncreaseBass,
    #[serde(rename = "Increase Treble")]
    IncreaseTreble,
    #[serde(rename = "Increase Vocals")]
    IncreaseVocals,
    Jazz,
    Latin,
    Loudness,
    Lounge,
    Piano,
    Pop,
    #[serde(rename = "R&B")]
    RhythmAndBlues,
    #[serde(rename = "Reduce Bass")]
    ReduceBass,
    #[serde(rename = "Reduce Treble")]
    ReduceTreble,
    Rock,
    #[serde(rename = "Small Speakers")]
    SmallSpeakers,
    #[serde(rename = "Spoken Word")]
    SpokenWord,
}
impl core::str::FromStr for EqualizerPreset {
    type Err = core::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "Acoustic" => EqualizerPreset::Acoustic,
            "Classical" => EqualizerPreset::Classical,
            "Dance" => EqualizerPreset::Dance,
            "Deep" => EqualizerPreset::Deep,
            "Electronic" => EqualizerPreset::Electronic,
            "Flat" => EqualizerPreset::Flat,
            "Hip-Hop" => EqualizerPreset::HipHop,
            "Increase Bass" => EqualizerPreset::IncreaseBass,
            "Increase Treble" => EqualizerPreset::IncreaseTreble,
            "Increase Vocals" => EqualizerPreset::IncreaseVocals,
            "Jazz" => EqualizerPreset::Jazz,
            "Latin" => EqualizerPreset::Latin,
            "Loudness" => EqualizerPreset::Loudness,
            "Lounge" => EqualizerPreset::Lounge,
            "Piano" => EqualizerPreset::Piano,
            "Pop" => EqualizerPreset::Pop,
            "R&B" => EqualizerPreset::RhythmAndBlues,
            "Reduce Bass" => EqualizerPreset::ReduceBass,
            "Reduce Treble" => EqualizerPreset::ReduceTreble,
            "Rock" => EqualizerPreset::Rock,
            "Small Speakers" => EqualizerPreset::SmallSpeakers,
            "Spoken Word" => EqualizerPreset::SpokenWord,

            _ => panic!("Unknown equalizer preset: {s}")
        })
    }
}
impl core::fmt::Display for EqualizerPreset {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut string = serde_json::to_string(self).unwrap();
        debug_assert_eq!(string.pop(), Some('"'));
        debug_assert_eq!(string.remove(0), '"');
        write!(f, "{string}")
    }
}

#[serde_as]
#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BasicTrack {
    /// The library's persistent ID for the track.
    /// This is a 16-character uppercase hexadecimal string.
    #[serde(rename = "persistentID")]
    pub persistent_id: String,

    /// The name of the track.
    pub name: String,

    /// The album of the track.
    #[serde(flatten)]
    pub album: TrackAlbum,

    /// The artist of the track.
    #[serde_as(as = "NoneAsEmptyString")]
    pub artist: Option<String>,

    /// The bitrate of the track, in kilobits per second.
    /// See also: `BasicTrack::sample_rate`.
    pub bitrate: Option<u16>,

    // The bookmarked position in the track, measured in seconds.
    pub bookmark: f32,

    /// Whether the playback position for this track can be remembered.
    #[serde(rename = "bookmarkable")]
    pub can_bookmark: bool,

    /// The tempo of this track, in beats per minute.
    #[serde_as(as = "DefaultOnError")]
    pub bpm: Option<core::num::NonZeroU16>,

    // TODO: Catalogue the ones used by Apple.
    /// The category of the track.
    #[serde_as(as = "NoneAsEmptyString")]
    pub category: Option<String>,

    /// The iCloud status of the track.
    #[serde(rename = "cloudStatus")]
    pub cloud_status: Option<TrackCloudStatus>,

    /// Freeform notes about the track.
    #[serde_as(as = "NoneAsEmptyString")]
    pub comment: Option<String>,

    /// Whether this track is from a compilation album.
    #[serde(rename = "compilation")]
    pub from_compilation: bool,

    /// The composer(s) of the track.
    #[serde_as(as = "NoneAsEmptyString")]
    pub composer: Option<String>,

    // TODO: Figure out if can be a u32.
    /// The common, unique ID for this track. If two tracks in different playlists have the same database ID, they are sharing the same data.
    #[serde(rename = "databaseID")]
    pub database_id: u64,

    /// The date the track was added to the library.
    /// Unavailable if, for example, a song were favorited without being "added".
    pub date_added: Option<Time>,

    /// The description of the track.
    #[serde_as(as = "NoneAsEmptyString")]
    pub description: Option<String>,

    /// The index of the disc containing this track on the source album.
    #[serde_as(as = "u8_ZeroAsNone")]
    pub disc_number: Option<core::num::NonZeroU8>, // not on audio streams; prob other stuff too

    /// Whether this track is disliked by the user.
    pub disliked: bool,

    /// Who, if anybody, downloaded this track.
    #[serde(flatten)]
    pub downloader: Option<TrackDownloader>,

    /// The length of the track, in seconds.
    #[serde(default)]
    #[serde_as(as = "optional_f32_duration")]
    pub duration: Option<core::time::Duration>,

    /// Whether this track is enabled for playback.
    pub enabled: bool,

    /// The episode details of this track; it's ID and episode number.
    #[serde(flatten)]
    pub episode_details: Option<EpisodeDetails>,

    /// The name of the equalizer preset set to be used for this track.
    #[serde(rename = "eq")]
    #[serde_as(as = "NoneAsEmptyString")]
    pub eq_preset: Option<EqualizerPreset>,

    /// The music/audio genre (category) of the track
    #[serde_as(as = "NoneAsEmptyString")]
    pub genre: Option<String>,

    /// The grouping (piece) of the track. Generally used to denote movements within a classical work.
    #[serde_as(as = "NoneAsEmptyString")]
    pub grouping: Option<String>,

    /// A text description of the audio file.
    /// 
    /// # Example
    /// - "MPEG audio file"
    /// - "Apple Music AAC audio file"
    #[serde_as(as = "NoneAsEmptyString")]
    pub kind: Option<String>, // `None` on a `urlTrack`

    /// The long description of the track.
    #[serde_as(as = "NoneAsEmptyString")]
    #[serde(default)]
    pub long_description: Option<String>,
    
    /// Whether this track is favorited.
    pub favorited: bool,

    /// The associated lyrics of the track. Does not work with lyrics from songs streamed by by Apple Music.
    #[serde_as(as = "NoneAsEmptyString")]
    #[serde(default)] // not present on `urlTrack`
    pub lyrics: Option<String>,

    /// The kind of media this track is considered.
    pub media_kind: MediaKind,

    // TODO: What does "modification" actually mean?
    /// The date at which this track was last modified.
    pub modification_date: Option<Time>,

    /// The name and index of this movement in the work, if applicable.
    #[serde(flatten)]
    #[serde_as(as = "MovementInfoOrNone")]
    pub movement: Option<MovementInfo>,

    /// The details on how many times and when this track was last played.
    #[serde(flatten)]
    pub played: PlayedInfo,

    /// Who, if anybody, "purchased" this track.
    /// Purchasing, in this case, can also mean just downloading for offline usage.
    // I don't have any actual iTunes purchased tracks to test on.
    #[serde(flatten)]
    pub purchaser: Option<TrackPurchaser>,

    /// The rating on the track.
    #[serde(flatten)]
    pub rating: Option<Rating>,

    /// The release date of this track.
    pub release_date: Option<Time>,
    
    /// The sample rate of the track, in hertz.
    /// Not available on audio streams.
    pub sample_rate: Option<u32>,

    // The season number of the track.
    pub season_number: Option<u16>,

    /// Whether this track can appear when shuffling.
    #[serde(rename = "shufflable")]
    pub can_appear_in_shuffles: bool,

    /// The details on how many times and when this track was last skipped.
    #[serde(flatten)]
    pub skipped: SkippedInfo,

    // ?
    /// The show name of the track/
    pub show: Option<String>,

    
    /// Sorting overrides for the track.
    #[serde(flatten)]
    pub sorting: SortingOverrides,

    /// The size of the track (in bytes).
    /// Unavailable for audio streams.
    #[serde(rename = "size")]
    pub bytes: Option<u64>,

    /// The playable portion of the track.
    /// A user is able to set the start and end regions of a track to be played.
    #[serde(flatten)]
    pub playable_range: Option<PlayableRange>,

    /// The length of the track in `MM:SS` format.
    /// Unavailable for audio streams.
    pub time: Option<String>,

    /// The index of the track on the source album.
    #[serde_as(as = "u16_ZeroAsNone")]
    pub track_number: Option<core::num::NonZeroU16>, // not on audio streams; prob other stuff too

    /// Relative volume adjustment of the track (-100% to 100%)
    pub volume_adjustment: i8,

    /// The work name of the track.
    #[serde_as(as = "NoneAsEmptyString")]
    pub work: Option<String>,

    /// The year the track was recorded/released
    #[serde_as(as = "u16_ZeroAsNone")]
    pub year: Option<core::num::NonZeroU16>,

    #[cfg(test)]
    #[serde(flatten)]
    non_directly_mapped: serde_json::Value,
}

#[serde_as]
#[derive(Deserialize, Serialize ,Debug)]
pub struct NetworkStreamTrack {
    /// The track details.
    #[serde(flatten)]
    pub track: BasicTrack,
    /// The address of the network stream.
    pub address: String,
}
impl core::ops::Deref for NetworkStreamTrack {
    type Target = BasicTrack;
    fn deref(&self) -> &Self::Target {
        &self.track
    }
}


#[serde_as]
#[derive(Deserialize, Serialize, Debug)]
pub struct LocalTrack {
    /// The track details.
    #[serde(flatten)]
    pub track: BasicTrack,
    /// The address of the network stream.
    pub address: String,
}
impl core::ops::Deref for LocalTrack {
    type Target = BasicTrack;
    fn deref(&self) -> &Self::Target {
        &self.track
    }
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(untagged)] // it doesn't wanna let me tag for whatever reason
pub enum Track {
    // #[serde(rename = "fileTrack")]
    NetworkStream(NetworkStreamTrack),
    // #[serde(rename = "urlTrack")]
    Local(LocalTrack),
    // #[serde(rename = "sharedTrack")]
    Shared(BasicTrack),
}
impl core::ops::Deref for Track {
    type Target = BasicTrack;
    fn deref(&self) -> &Self::Target {
        match self {
            Track::NetworkStream(v) => &v.track,
            Track::Local(v) => &v.track,
            Track::Shared(v) => v,
        }
    }
}
impl From<Track> for BasicTrack {
    fn from(val: Track) -> Self {
        match val {
            Track::NetworkStream(v) => v.track,
            Track::Local(v) => v.track,
            Track::Shared(v) => v,
        }
    }
}
impl Track {
    /// Fetches and returns the currently playing song.
    /// If you find yourself doing this repeatedly, consider using [`Session`](crate::Session) instead.
    pub async fn get_now_playing() -> Result<Option<Self>, crate::error::SingleEvaluationError> {
        osascript::run::<[&str; 0], _>("JSON.stringify(Application(\"Music\").currentTrack().properties())", osascript::Language::JavaScript, [])
            .await
            .map_err(crate::error::SingleEvaluationError::IoFailure)
            .and_then(|output| { Ok(serde_json::from_str(&output.stdout())?) })
    }
}

#[serde_as]
#[derive(Debug, Deserialize, Serialize ,PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TrackAlbum {
    /// The name of the album that this track is in.
    #[serde(rename = "album")]
    #[serde_as(as = "NoneAsEmptyString")]
    pub name: Option<String>,

    /// The artist(s) of the album for this track.
    #[serde(rename = "albumArtist")]
    #[serde_as(as = "NoneAsEmptyString")]
    pub artist: Option<String>,

    /// Is the album for this track disliked?
    #[serde(rename = "albumDisliked")]
    pub disliked: bool,

    /// Is the album for this track favorited?
    #[serde(rename = "albumFavorited")]
    pub favorited: bool,
    
    /// The rating of the album for this track (0 to 100)
    #[serde(rename = "albumRating", flatten)]
    pub rating: Option<rating::ForTrackAlbum>,

    /// The number of movements in the work
    pub movement_count: u16,

    /// The number of tracks on this track's album.
    pub track_count: u16,

    /// The number of discs in this track's album.
    #[serde_as(as = "u8_ZeroAsNone")]
    pub disc_count: Option<NonZeroU8>,

    /// Whether this track is from a gapless album.
    #[serde(default)]
    pub gapless: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn track_album() {
        assert_eq!(serde_json::from_str::<TrackAlbum>(r#"{
            "album": "Album Name",
            "albumArtist": "Album Artist",
            "albumDisliked": false,
            "albumFavorited": true,
            "albumRating": 55,
            "albumRatingKind": "user",
            "discCount": 0,
            "trackCount": 1,
            "gapless": false,
            "movementCount": 0
        }"#).unwrap(), TrackAlbum {
            name: Some("Album Name".to_owned()),
            artist: Some("Album Artist".to_owned()),
            disliked: false,
            favorited: true,
            rating: Some(Rating::User(55).into()),
            disc_count: None,
            track_count: 1,
            gapless: false,
            movement_count: 0
        });
    }

    #[tokio::test]
    #[ignore = "must be manually run with the correct environment setup"]
    async fn test_real_world()  {
        let track = Track::get_now_playing().await;
        println!("{track:#?}");
        assert!(track.is_ok());
    }

    #[test]
    fn parse_pending_connection_track() {
        let data = r#"{
            "class": "urlTrack",
            "id": 63093,
            "index": 1,
            "name": "Connecting…",
            "persistentID": "9C7E988AD00DBDFF",
            "databaseID": 63089,
            "dateAdded": "2025-08-24T08:03:23.000Z",
            "artist": "",
            "albumArtist": "",
            "composer": "",
            "album": "",
            "genre": "",
            "trackCount": 0,
            "trackNumber": 0,
            "discCount": 0,
            "discNumber": 0,
            "volumeAdjustment": 0,
            "year": 0,
            "comment": "",
            "eq": "",
            "kind": "",
            "mediaKind": "song",
            "enabled": true,
            "start": 0,
            "finish": 0,
            "playedCount": 0,
            "skippedCount": 0,
            "compilation": false,
            "rating": 0,
            "bpm": 0,
            "grouping": "",
            "bookmarkable": false,
            "bookmark": 0,
            "shufflable": true,
            "category": "",
            "description": "",
            "episodeNumber": 0,
            "unplayed": true,
            "sortName": "",
            "sortAlbum": "",
            "sortArtist": "",
            "sortComposer": "",
            "sortAlbumArtist": "",
            "favorited": false,
            "disliked": false,
            "albumFavorited": false,
            "albumDisliked": false,
            "work": "",
            "movement": "",
            "movementNumber": 0,
            "movementCount": 0
        }"#;

        let de: Result<Track, _> = serde_json::from_str(data);
        assert!(de.is_ok(), "track did not deserialize");
    }
}
