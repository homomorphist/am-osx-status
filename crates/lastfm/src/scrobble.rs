/// Details on a track that was listened to or is currently being listened to.
/// 
/// It is the shared parameters of the following endpoints:
/// - <https://www.last.fm/api/show/track.scrobble#Params>
/// - <https://www.last.fm/api/show/track.updateNowPlaying#Params>
#[derive(Default)] // Should only be used for `{ artist: Foo, track: Bar, ..Default::default() }`, not generating an entire default.
pub struct HeardTrackInfo<'a> {
    /// The artist name.
    pub artist: &'a str,

    /// The track name.
    pub track: &'a str,

    /// The track number of the track on the album.
    pub track_number: Option<u32>,

    /// The album name.
    pub album: Option<&'a str>,

    /// The album artist, if it differs from the track artist
    pub album_artist: Option<&'a str>,

    /// The MusicBrainz Track ID.
    // TODO: Gate this type definition behind a feature, making it a `&'a str` otherwise.
    pub mbid: Option<brainz::music::Id<brainz::music::entities::Track>>,

    /// The duration of the track in seconds.
    pub duration_in_seconds: Option<u32>,
}
impl<'a> HeardTrackInfo<'a> {
    pub fn promote_to_scrobble(self, parameters: ScrobbleEnrichmentParameters) -> Scrobble<'a> where Self: 'a {
        Scrobble {
            info: self,
            timestamp: parameters.timestamp,
            chosen_by_user: parameters.chosen_by_user
        }
    }
}

/// <https://www.last.fm/api/show/track.scrobble#Params>
pub struct Scrobble<'a> {
    /// The track that was played.
    pub info: HeardTrackInfo<'a>,

    /// The time the track started playing.
    // TODO: Just use a `u64`?
    pub timestamp: chrono::DateTime<chrono::Utc>,
    
    /// Whether the user chose to play this song.
    /// If `false`, the song was chosen by someone else, such as a radio station or recommendation service.
    /// If there is any ambiguity or doubt, then don't send this value. Defaults to `true`.
    pub chosen_by_user: Option<bool>,
}


pub struct ScrobbleEnrichmentParameters {
    /// The time the track started playing.
    // TODO: Just use a `u64`?
    pub timestamp: chrono::DateTime<chrono::Utc>,
    
    /// Whether the user chose to play this song.
    /// If `false`, the song was chosen by someone else, such as a radio station or recommendation service.
    /// Defaults to `true`.
    pub chosen_by_user: Option<bool>,
}

pub use response::ScrobbleError;
pub mod response {
    use maybe_owned_string::MaybeOwnedString;

    use super::*;

    pub struct ScrobbleServerResponse<'a> {
        json: core::pin::Pin<String>,
        pub results: Vec<Result<TimestampedAcknowledgement<MaybeOwnedString<'a>>, ScrobbleError>>,
        pub counts: raw::ResponseAttributes,
    }
    impl<'a> ScrobbleServerResponse<'a> {
        pub fn new(json: String, capacity: usize) -> Result<Self, serde_json::Error> {
            let json = core::pin::Pin::new(json);
    
            let (results, counts) = {
                // Disassociate to allow reference into JSON despite it "moving" into this struct.
                // It's alright since it's pinned and doesn't actually move.
                let json = unsafe {
                    // These are all zero-cost operations :)
                    let bytes = core::slice::from_raw_parts(json.as_ptr(), json.len());
                    let bytes = core::mem::transmute::<&'a [u8], &[u8]>(bytes);
                    core::str::from_utf8_unchecked(bytes)
                };

                let mut vec = Vec::with_capacity(capacity);
                let raw: raw::Response = serde_json::from_str(json)?;
                let counts = raw.scrobbles.counts;
                let raw: Vec<_> = raw.scrobbles.inner.into();

                for response in raw {
                    if let Some(code) = response.ignored_message.code {
                        vec.push(Err(code))
                    } else {
                        let timestamp = response.timestamp.parse().expect("bad timestamp");
                        let mut album: Option<MaybeCorrected<MaybeOwnedString>> = None;
                        let mut album_artist: Option<MaybeCorrected<MaybeOwnedString>> = None;

                        let track = MaybeCorrected {
                            value: response.track.text,
                            corrected: response.track.corrected
                        };
                        let artist = MaybeCorrected {
                            value: response.artist.text,
                            corrected: response.artist.corrected
                        };

                        
                        if !response.album.text.is_empty() {
                            album = Some(MaybeCorrected {
                                value: response.album.text,
                                corrected: response.album.corrected,
                            });
                        }
                        if !response.album_artist.text.is_empty() {
                            album_artist = Some(MaybeCorrected {
                                value: response.album_artist.text,
                                corrected: response.album_artist.corrected
                            });
                        }
                    
                        vec.push(Ok(TimestampedAcknowledgement {
                            ack: Acknowledgement {
                                artist,
                                track,
                                album,
                                album_artist,
                            },
                            timestamp
                        }))
                    }
                }

                (vec, counts)
            };
            
            Ok(Self {
                json,
                results,
                counts
            })
        }
    }
    
    pub struct ServerUpdateNowPlayingResponse<'a> {
        json: core::pin::Pin<String>,
        pub result: Acknowledgement<MaybeOwnedString<'a>>,
    }
    impl<'a> ServerUpdateNowPlayingResponse<'a> {
        pub fn new(json: String) -> Result<Self, serde_json::Error> {
            let json = core::pin::Pin::new(json);

            let acknowledgement = {
                // Disassociate to allow reference into JSON despite it "moving" into this struct.
                // It's alright since it's pinned and doesn't actually move.
                let json = unsafe {
                    // These are all zero-cost operations :)
                    let bytes = core::slice::from_raw_parts(json.as_ptr(), json.len());
                    let bytes = core::mem::transmute::<&'a [u8], &[u8]>(bytes);
                    core::str::from_utf8_unchecked(bytes)
                };

                let response = serde_json::from_str::<raw::NowPlayingResponse>(json)?.inner;

                let track = MaybeCorrected {
                    value: response.track.text,
                    corrected: response.track.corrected
                };
                let artist = MaybeCorrected {
                    value: response.artist.text,
                    corrected: response.artist.corrected
                };

                let mut album: Option<MaybeCorrected<MaybeOwnedString>> = None;
                let mut album_artist: Option<MaybeCorrected<MaybeOwnedString>> = None;
                
                if !response.album.text.is_empty() {
                    album = Some(MaybeCorrected {
                        value: response.album.text,
                        corrected: response.album.corrected,
                    });
                }
                if !response.album_artist.text.is_empty() {
                    album_artist = Some(MaybeCorrected {
                        value: response.album_artist.text,
                        corrected: response.album_artist.corrected
                    });
                }

                Acknowledgement {
                    artist,
                    track,
                    album,
                    album_artist,
                }
            };
          
            Ok(Self {
                json: core::pin::Pin::new(json.to_string()),
                result: acknowledgement,
            })
        }
    }

    /// <https://www.last.fm/api/show/track.scrobble#Attributes>
    #[derive(PartialEq, Debug, thiserror::Error)]
    pub enum ScrobbleError {
        /// Scrobble was ignored because the artist name is blacklisted.
        #[error("ignored because artist name")]
        BadArtist, // Also occurs upon outlandish timestamp (i.e. when passing milliseconds instead of seconds); I dunno how to represent that here.

        /// Scrobble was ignored because the track name is blacklisted.
        #[error("ignored because track name")]
        BadTrack,

        /// Scrobble was ignored because the timestamp is too old.
        #[error("timestamp is too old")]
        TimestampTooOld,

        /// Scrobble was ignored because the timestamp is too new (i.e. it's in the future?).
        #[error("timestamp is too new")]
        TimestampTooNew,

        /// Scrobble was ignored because the daily limit was reached.
        #[error("scrobble daily limit reached")]
        DailyLimitReached
    }
    impl TryFrom<u8> for ScrobbleError {
        type Error = InvalidScrobbleErrorCodeError;
        fn try_from(value: u8) -> Result<Self, Self::Error> {
            match value {
                1 => Ok(Self::BadArtist),
                2 => Ok(Self::BadTrack),
                3 => Ok(Self::TimestampTooOld),
                4 => Ok(Self::TimestampTooNew),
                5 => Ok(Self::DailyLimitReached),
                _ => Err(InvalidScrobbleErrorCodeError { code: value })
            }
        }
    }

    #[repr(transparent)]
    pub struct InvalidScrobbleErrorCodeError { code: u8 }
    impl core::fmt::Debug for InvalidScrobbleErrorCodeError {
        fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
            write!(f, "invalid scrobble error code: {}", self.code)
        }
    }
    impl core::fmt::Display for InvalidScrobbleErrorCodeError {
        fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
            <Self as core::fmt::Debug>::fmt(self, f)
        }
    }
    impl std::error::Error for InvalidScrobbleErrorCodeError {}


    #[derive(PartialEq)]
    pub struct MaybeCorrected<T> {
        pub corrected: bool,
        pub value: T
    }

    pub struct Acknowledgement<S> {
        artist: MaybeCorrected<S>,
        track: MaybeCorrected<S>,
        album: Option<MaybeCorrected<S>>,
        album_artist: Option<MaybeCorrected<S>>,
    }

    pub struct TimestampedAcknowledgement<S> {
        ack: Acknowledgement<S>,
        timestamp: u32,
    }


    pub(crate) mod raw {
        use super::*;
        use maybe_owned_string::MaybeOwnedString;
        use serde::*;

        #[derive(Debug, Deserialize)]
        #[serde(untagged)]
        pub enum MaybeMany<T> {
            Many(Vec<T>),
            One(T)
        }
        impl<T> From<MaybeMany<T>> for Vec<T> {
            fn from(val: MaybeMany<T>) -> Self {
                match val {
                    MaybeMany::Many(vec) => vec,
                    MaybeMany::One(single) => vec![single]
                }
            }
        }

        #[derive(Debug, Deserialize)]
        pub struct MaybeCorrected<'a> {
            #[serde(borrow, rename = "#text", default)]  // not present on albums if omitted but always present on album artist if omitted ?? idk. regardless, empty string means omitted
            pub text: MaybeOwnedString<'a>,
            #[serde(deserialize_with = "deserialize_numeric_bool_string")]
            pub corrected: bool,
        }

        fn deserialize_numeric_bool_string<'de, D>(deserializer: D) -> Result<bool, D::Error> where D: serde::Deserializer<'de>, {
            let str= serde::Deserialize::deserialize(deserializer)?;
            match str {
                "0" => Ok(false),
                "1" => Ok(true),
                _ => Err(serde::de::Error::custom("unexpected value for numeric bool")),
            }
        }

        fn deserialize_stringified_u8<'de, D>(deserializer: D) -> Result<u8, D::Error> where D: serde::Deserializer<'de>, {
            let str: &str = serde::Deserialize::deserialize(deserializer)?;
            str.parse().map_err(serde::de::Error::custom)
        }

        #[derive(Debug)]
        pub struct IgnoredMessage {
            pub code: Option<ScrobbleError>,
        }
        impl<'de> serde::Deserialize<'de> for IgnoredMessage {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: serde::Deserializer<'de>, {
                #[derive(Deserialize)]
                struct Untyped {
                    #[serde(deserialize_with = "deserialize_stringified_u8")]
                    code: u8
                }

                let helper = Untyped::deserialize(deserializer)?;
                let code = if helper.code == 0 { None } else {
                    Some(ScrobbleError::try_from(helper.code).map_err(serde::de::Error::custom)?)
                };

                Ok(IgnoredMessage { code })
            }
        }

        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct NowPlayingResponseInner<'a> {
            #[serde(borrow)] pub artist: MaybeCorrected<'a>,
            #[serde(borrow)] pub track: MaybeCorrected<'a>,
            #[serde(borrow)] pub album: MaybeCorrected<'a>,
            #[serde(borrow)] pub album_artist: MaybeCorrected<'a>,
            pub ignored_message: IgnoredMessage,
        }

        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct NowPlayingResponse<'a> {
            #[serde(borrow, rename = "nowplaying")]
            pub inner: NowPlayingResponseInner<'a>
        }

        // man, sometimes i wish rust had a *teeny* bit more inheritance
        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct ScrobbleResponse<'a> {
            #[serde(borrow)] pub artist: MaybeCorrected<'a>,
            #[serde(borrow)] pub track: MaybeCorrected<'a>,
            #[serde(borrow)] pub album: MaybeCorrected<'a>,
            #[serde(borrow)] pub album_artist: MaybeCorrected<'a>,
            /// Unix timestamp in seconds.
            #[serde(borrow)] pub timestamp: &'a str,
            pub ignored_message: IgnoredMessage,
        }

        #[derive(Debug, Deserialize)]
        pub struct ResponseAttributes {
            pub ignored: usize,
            pub accepted: usize,
        }

        #[derive(Debug, Deserialize)]
        pub struct ScrobbleContainer<'a> {
            #[serde(rename = "scrobble", borrow)] pub inner: MaybeMany<ScrobbleResponse<'a>>,
            #[serde(rename = "@attr")] pub counts: ResponseAttributes
        }

        #[derive(Debug, Deserialize)]
        pub struct Response<'a> {
            #[serde(borrow)] pub scrobbles: ScrobbleContainer<'a>,
        }
    }
}
