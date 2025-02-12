#![allow(unused)]

use std::{default, num::NonZeroU8};

use serde_with::*;
use serde::Deserialize;

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


#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")] // with space delimiters
enum TrackCloudStatus {
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

#[derive(Debug, Deserialize)]
struct TrackDownloader {
    /// The Apple ID of the person who downloaded this track.
    #[serde(rename = "downloaderAppleID")]
    apple_id: String,
    /// The name of the person who downloaded this track.
    #[serde(rename = "downloaderName")]
    name: String,
}

#[derive(Debug, Deserialize)]
struct TrackPurchaser {
    /// The Apple ID of the person who purchased this track.
    #[serde(rename = "purchaserAppleID")]
    apple_id: String,
    /// The name (or email) of the person who purchased this track.
    #[serde(rename = "purchaserName")]
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum MediaKind {
    Song,
    #[serde(rename = "music video")]
    MusicVideo,

    Unknown
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EpisodeDetails {
    /// The episode ID of the track.
    #[serde(rename = "episodeID")]
    pub episode_id: String,

    /// The episode number of the track.
    pub episode_number: u16,
}


#[derive(Debug, Deserialize)]
#[serde(bound = "D: Deserialize<'de>")]
struct PlayedInfo<D> {
    /// Number of times this track has been played.
    #[serde(rename = "playedCount")]
    times: u32,

    /// The date and time this track was last played.
    #[serde(rename = "playedDate")]
    last: Option<D>,

    /// Whether this track has never been played before.
    #[serde(rename = "unplayed")]
    never: bool
}

#[derive(Debug, Deserialize)]
#[serde(bound = "D: Deserialize<'de>")]
struct SkippedInfo<D> {
    /// Number of times this track has been skipped.
    #[serde(rename = "skippedCount")]
    times: u32,

    /// The date and time this track was last skipped.
    #[serde(rename = "skippedDate")]
    last: Option<D>,
}

#[derive(Debug, Deserialize, serde::Serialize, Default, Clone)]
struct MovementInfo {
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
        dbg!(&value);
       if value.name.is_empty() {
            Ok(None)
        } else {
            Ok(Some(value))
        }
    }
);


#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SortingOverrides {
    /// Override string to use for the track when sorting by album
    #[serde_as(as = "NoneAsEmptyString")]
    #[serde(rename = "sortAlbum")]
    album: Option<String>,

    /// Override string to use for the track when sorting by artist
    #[serde_as(as = "NoneAsEmptyString")]
    #[serde(rename = "sortArtist")]
    artist: Option<String>,

    /// Override string to use for the track when sorting by album artist
    #[serde_as(as = "NoneAsEmptyString")]
    #[serde(rename = "sortAlbumArtist")]
    album_artist: Option<String>,

    /// Override string to use for the track when sorting by name
    #[serde_as(as = "NoneAsEmptyString")]
    #[serde(rename = "sortName")]
    name: Option<String>,

    /// Override string to use for the track when sorting by composer
    #[serde_as(as = "NoneAsEmptyString")]
    #[serde(rename = "sortComposer")]
    composer: Option<String>,

    /// Override string to use for the track when sorting by show name
    #[serde_as(as = "NoneAsEmptyString")]
    #[serde(rename = "sortShow", default)]
    show: Option<String>,
}


#[derive(Debug, Deserialize)]
struct PlayableRange {
    /// The start of the playable region, in seconds. Defaults to zero.
    pub start: f32,

    /// The end of the playable region, in seconds. Defaults to the duration of the song.
    #[serde(rename = "finish")]
    pub end: f32,
}

#[derive(Debug, Deserialize)]
enum EqualizerPreset {
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

            _ => panic!("Unknown equalizer preset: {}", s)
        })
    }
}

#[serde_as]
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase", bound = "D: Deserialize<'de>")] 
struct BasicTrack<D> {
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
    pub date_added: Option<D>, // TODO: 

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
    /// Unavailable for audio streams.
    pub duration: Option<f32>,

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
    pub modification_date: Option<D>,

    /// The name and index of this movement in the work, if applicable.
    #[serde(flatten)]
    #[serde_as(as = "MovementInfoOrNone")]
    pub movement: Option<MovementInfo>,

    /// The details on how many times and when this track was last played.
    #[serde(flatten)]
    pub played: PlayedInfo<D>,

    /// Who, if anybody, "purchased" this track.
    /// Purchasing, in this case, can also mean just downloading for offline usage.
    // I don't have any actual iTunes purchased tracks to test on.
    #[serde(flatten)]
    pub purchaser: Option<TrackPurchaser>,

    /// The rating on the track.
    #[serde(flatten)]
    pub rating: Option<crate::Rating>,

    /// The release date of this track.
    pub release_date: Option<D>,
    
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
    pub skipped: SkippedInfo<D>,

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
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase", bound = "D: Deserialize<'de>")]
struct NetworkStreamTrack<D> {
    /// The track details.
    #[serde(flatten)]
    pub track: BasicTrack<D>,
    /// The address of the network stream.
    pub address: String,
}
impl<D> core::ops::Deref for NetworkStreamTrack<D> {
    type Target = BasicTrack<D>;
    fn deref(&self) -> &Self::Target {
        &self.track
    }
}


#[serde_as]
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase", bound = "D: Deserialize<'de>")]
struct LocalTrack<D> {
    /// The track details.
    #[serde(flatten)]
    pub track: BasicTrack<D>,
    /// The address of the network stream.
    pub address: String,
}
impl<D> core::ops::Deref for LocalTrack<D> {
    type Target = BasicTrack<D>;
    fn deref(&self) -> &Self::Target {
        &self.track
    }
}

#[derive(Deserialize, Debug)]
#[serde(untagged)] // it doesn't wanna let me tag for whatever reason
enum TrackVariant<D> {
    #[serde(rename = "fileTrack")]
    NetworkStream(NetworkStreamTrack<D>),
    #[serde(rename = "urlTrack")]
    Local(LocalTrack<D>),
    #[serde(rename = "sharedTrack")]
    Shared(BasicTrack<D>),
}
impl<D> core::ops::Deref for TrackVariant<D> {
    type Target = BasicTrack<D>;
    fn deref(&self) -> &Self::Target {
        match self {
            TrackVariant::NetworkStream(v) => &v.track,
            TrackVariant::Local(v) => &v.track,
            TrackVariant::Shared(v) => v,
        }
    }
}

#[serde_as]
#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct TrackAlbum {
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
    pub rating: Option<crate::rating::ForTrackAlbum>,

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
    use crate::Rating;
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
    #[ignore = "must be explicitly run"]
    async fn test_real()  {
        type Date = chrono::DateTime<chrono::Utc>;
        use osascript::{Session, ReplOutput};
        let mut session = Session::javascript().await.expect("cannot create");
        let result = session.run("JSON.stringify(Application(\"Music\").selection()[0].properties())").await.unwrap();
        dbg!(result.raw.as_lossy_str());
        let result = result.guess().unwrap();
        let result = &result[1..result.len()-1]; // remove quotes
        let result = unescape::unescape(result).unwrap();
        println!("{}", result);
        let result: TrackVariant<Date> = serde_json::from_str(&result).unwrap();
        println!("{:#?}", result);
    }

    #[tokio::test]
    #[ignore = "must be explicitly run"]
    async fn tesdt_real()  {
        type Date = chrono::DateTime<chrono::Utc>;
        use osascript::{Session, ReplOutput};
        let mut session = Session::javascript().await.expect("cannot create");
        let result = session.run("JSON.stringify(Application(\"Music\").properties())").await.unwrap();
        dbg!(result.raw.as_lossy_str());
        let result = result.guess().unwrap();
        let result = &result[1..result.len()-1]; // remove quotes
        let result = unescape::unescape(result).unwrap();
        println!("{}", result);

    }
}

