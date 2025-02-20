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



pub mod response {
    use super::*;

    pub struct ScrobbleServerResponse<'a> {
        json: core::pin::Pin<String>,
        pub results: Vec<Result<Acknowledgement<'a>, ScrobbleError>>,
        pub counts: raw::ResponseAttributes,
    }
    impl<'a> ScrobbleServerResponse<'a> {
        pub fn new(json: String, capacity: usize) -> Self {
            let json = core::pin::Pin::new(json);
    
            let (results, counts) = {
                // Disassociate to allow reference into JSON despite it "moving" into this struct.
                // It's alright since it's pinned and doesn't actually move.
                let json = unsafe {
                    let bytes = core::slice::from_raw_parts(json.as_ptr(), json.len());
                    let bytes = core::mem::transmute::<&'a [u8], &[u8]>(bytes);
                    core::str::from_utf8_unchecked(bytes)
                };

                // TODO: I'm not actually handling any errors! Uh oh!
                let raw: raw::Response = serde_json::from_str(json).expect("cannot deserialize");
                let mut vec = Vec::with_capacity(capacity);
                let raw = raw.scrobbles; let counts = raw.counts;
                let raw = match raw.inner {
                    raw::MaybeMany::One(single) => vec![single],
                    raw::MaybeMany::Many(many) => many
                };

                for response in raw {
                    if response.ignored_message.code != "0" {
                        let code: u8 = response.ignored_message.code.parse().expect("could not parse ignoration code");
                        vec.push(Err(ScrobbleError::try_from(code).expect("unknown ignoration code")))
                    } else {
                        let timestamp = response.timestamp.parse().expect("bad timestamp");
                        let mut album: Option<MaybeCorrected<&str>> = None;
                        let mut album_artist: Option<MaybeCorrected<&str>> = None;

                        let track = MaybeCorrected {
                            value: response.track.text,
                            corrected: response.track.corrected == "1",
                        };
                        let artist = MaybeCorrected {
                            value: response.artist.text,
                            corrected: response.artist.corrected == "1",
                        };

                        if !response.album.text.is_empty() {
                            album = Some(MaybeCorrected {
                                value: response.album.text,
                                corrected: response.album.corrected == "1",
                            });
                        }
                        if !response.album_artist.text.is_empty() {
                            album_artist = Some(MaybeCorrected {
                                value: response.album_artist.text,
                                corrected: response.album_artist.corrected == "1",
                            });
                        }
                    
                        vec.push(Ok(Acknowledgement {
                            artist,
                            track,
                            album,
                            album_artist,
                            timestamp
                        }))
                    }
                }

                (vec, counts)
            };
            
            Self {
                json,
                results,
                counts
            }
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
        type Error = ();
        fn try_from(value: u8) -> Result<Self, Self::Error> {
            match value {
                1 => Ok(Self::BadArtist),
                2 => Ok(Self::BadTrack),
                3 => Ok(Self::TimestampTooOld),
                4 => Ok(Self::TimestampTooNew),
                5 => Ok(Self::DailyLimitReached),
                _ => Err(())
            }
        }
    }


    #[derive(PartialEq)]
    pub struct MaybeCorrected<T> {
        pub corrected: bool,
        pub value: T
    }

    pub struct Acknowledgement<'a> {
        artist: MaybeCorrected<&'a str>,
        track: MaybeCorrected<&'a str>,
        album: Option<MaybeCorrected<&'a str>>,
        album_artist: Option<MaybeCorrected<&'a str>>,
        timestamp: u32,
    }


    pub(crate) mod raw {
        use super::*;
        use serde::*;


        #[derive(Debug, Deserialize)]
        #[serde(untagged)]
        pub enum MaybeMany<T> {
            Many(Vec<T>),
            One(T)
        }

        #[derive(Debug, Deserialize)]
        pub struct MaybeCorrected<'a> {
            #[serde(rename = "#text", default)]  // not present on albums if omitted but always present on album artist if omitted ?? idk. regardless, empty string means omitted
            pub text: &'a str,
            pub corrected: &'a str, // "0" || "1",
        }

        #[derive(Debug, Deserialize)]
        pub struct IgnoredMessage<'a> {
            pub code: &'a str // u8
        }

        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct ScrobbleResponse<'a> {
            #[serde(borrow)] pub artist: MaybeCorrected<'a>,
            #[serde(borrow)] pub track: MaybeCorrected<'a>,
            #[serde(borrow)] pub album: MaybeCorrected<'a>,
            #[serde(borrow)] pub album_artist: MaybeCorrected<'a>,
            #[serde(borrow)] pub timestamp: &'a str, // int (seconds past unix epoch)
            #[serde(borrow)] pub ignored_message: IgnoredMessage<'a>,
        }

        #[derive(Debug, Deserialize)]
        pub struct ResponseAttributes {
            pub ignored: usize,
            pub accepted: usize,
        }

        #[derive(Debug, Deserialize)]
        pub struct ScrobblesContainer<'a> {
            #[serde(rename = "scrobble", borrow)] pub inner: MaybeMany<ScrobbleResponse<'a>>,
            #[serde(rename = "@attr")] pub counts: ResponseAttributes
        }

        #[derive(Debug, Deserialize)]
        pub struct Response<'a> {
            #[serde(borrow)] pub scrobbles: ScrobblesContainer<'a>,
        }
    }
}



