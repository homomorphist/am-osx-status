use maybe_owned_string::MaybeOwnedString;

use crate::{auth, scrobble};

pub enum Value {
    String(String),
    Boolean(bool),
    Timestamp(chrono::DateTime<chrono::Utc>),
}


#[derive(serde::Serialize)]
#[serde(transparent)]
pub struct Map<'a>(pub std::collections::HashMap<String, MaybeOwnedString<'a>>);
impl<'a> Map<'a> {
    pub fn from_collection(collection: std::collections::HashMap<String, MaybeOwnedString<'a>>) -> Self {
        Self(collection)
    }

    pub fn add(&mut self, key: String, value: MaybeOwnedString<'a>) {
        self.0.insert(key, value);
    }

    fn get_ordered_keys(&self) -> Vec<&str> {
        let mut ordered_keys = Vec::<&str>::with_capacity(self.0.len());
        for key in self.0.keys() {
            ordered_keys.push(key);
        }
        ordered_keys.sort();
        ordered_keys
    }

    pub fn sign(&self, session_key: &crate::auth::SessionKey, identity: &auth::ClientIdentity) -> crate::auth::ApiSignature {
        let mut built = String::new();

        for key in self.get_ordered_keys() {
            built += key;
            built += self.0.get(key).expect("key without value")
        }

        built += identity.get_secret();
        
        let hex = format!("{:x}", md5::compute(built));
        let hex = crate::auth::internal::ThirtyTwoCharactersLowercaseHexAsciiString::new(&hex).expect("badly formatted signature");
        crate::auth::ApiSignature(hex)
    }
}


impl<'a> From<&'a scrobble::HeardTrackInfo<'a>> for Map<'a> {
    fn from(track: &'a scrobble::HeardTrackInfo) -> Self {
        const MIN_PARAMETER_COUNT: usize = 2; // Track, Album
        let mut map: std::collections::HashMap<String, MaybeOwnedString<'_>> = std::collections::HashMap::with_capacity(MIN_PARAMETER_COUNT);
        map.insert("artist".to_owned(), MaybeOwnedString::Borrowed(track.artist));
        map.insert("track".to_owned(), MaybeOwnedString::Borrowed(track.track));
        if let Some(album) = track.album { map.insert("album".to_owned(), MaybeOwnedString::Borrowed(album)); }
        if let Some(mbid) = &track.mbid { map.insert("mbid".to_owned(), MaybeOwnedString::Borrowed(mbid.as_str())); }
        if let Some(album_artist) = track.album_artist { map.insert("albumArtist".to_owned(), MaybeOwnedString::Borrowed(album_artist)); }
        if let Some(duration) = track.duration_in_seconds { map.insert("duration".to_owned(), MaybeOwnedString::Owned(duration.to_string())); }
        Self(map)
    }
}

impl<'a> From<&'a [scrobble::Scrobble<'a>]> for Map<'a> {
    fn from(scrobbles: &'a [scrobble::Scrobble]) -> Self {
        const MIN_PARAMETER_COUNT: usize = 3; // Track, Album, Timestamp
        let mut map = std::collections::HashMap::with_capacity(MIN_PARAMETER_COUNT * scrobbles.len());
        for (i, scrobble) in scrobbles.iter().enumerate() {
            map.insert(format!("artist[{i}]"), MaybeOwnedString::Borrowed(scrobble.info.artist));
            map.insert(format!("track[{i}]"), MaybeOwnedString::Borrowed(scrobble.info.track));
            map.insert(format!("timestamp[{i}]"), MaybeOwnedString::Owned(scrobble.timestamp.timestamp().to_string()));
            if let Some(album) = scrobble.info.album { map.insert(format!("album[{i}]"), MaybeOwnedString::Borrowed(album)); }
            if let Some(chosen) = scrobble.chosen_by_user { map.insert(format!("chosenByUser[{i}]"), MaybeOwnedString::Borrowed(if chosen { "1" } else { "0" })); }
            if let Some(mbid) = &scrobble.info.mbid { map.insert(format!("mbid[{i}]"), MaybeOwnedString::Borrowed(mbid.as_str())); }
            if let Some(album_artist) = scrobble.info.album_artist { map.insert(format!("albumArtist[{i}]"), MaybeOwnedString::Borrowed(album_artist)); }
            if let Some(duration) = scrobble.info.duration_in_seconds { map.insert(format!("duration[{i}]"), MaybeOwnedString::Owned(duration.to_string())); }
        } 
        Self(map)
    }
}

