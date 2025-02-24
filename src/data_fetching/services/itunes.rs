#![allow(unused)]
use serde::Deserialize;

use unicode_normalization::UnicodeNormalization;

const ITUNES_SEARCH_BASE_URL: &str = "https://itunes.apple.com/search";

fn normalize(string: &str) -> String {
    string.trim().nfkc().collect::<String>().to_lowercase()
}


#[derive(thiserror::Error, Debug)]
pub enum SongSearchError {
    #[error("could not get data from iTunes")]
    Network(reqwest::Error),
    #[error("could not decode iTunes response")]
    TextDecoding(reqwest::Error),
    #[error("could not deserialize iTunes response")]
    Deserialization(#[from] serde_json::Error)
}

async fn search_songs(query: &str) -> Result<Vec<ITunesStoreSong>, SongSearchError> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ITunesSongSearchOutcome {
        #[allow(unused)]
        result_count: i32,
        results: Vec<ITunesStoreSong>,
    }

    let query = urlencoding::encode(query);
    let url = format!("{}?term={}&entity=song&limit=50", ITUNES_SEARCH_BASE_URL, query);
    let res = reqwest::get(url).await.map_err(SongSearchError::Network)?;
    let text = res.text().await.map_err(SongSearchError::TextDecoding)?;
    serde_json::from_str::<ITunesSongSearchOutcome>(&text)
        .map(|outcome| outcome.results)
        .map_err(SongSearchError::Deserialization)
}

// TODO: Rank with numeric. With Levenshtein; after removing parentheses, ignoring album, stuff like that.
fn does_track_match_search(track: &crate::status_backend::DispatchableTrack, found: &ITunesStoreSong) -> bool {
    let name = normalize(&track.name);
    let artist = normalize(&track.artist.clone().unwrap_or_default());
    let collection = normalize(&track.album.clone().unwrap_or_default());

    (
        normalize(&found.name) == name ||
        normalize(&found.name_censored) == name
    )
        && (normalize(&found.artist_name) == artist)
        && (normalize(&found.collection_name) == collection)
}

pub async fn find_track(track: &crate::status_backend::DispatchableTrack) -> Result<Option<ITunesStoreSong>, SongSearchError> {
    let query = format!("{} {}", track.artist.clone().unwrap_or_default(), track.name);
    let songs = search_songs(&query).await?;
    if songs.len() == 1 { return Ok(songs.into_iter().next()) }
    Ok(songs.into_iter().find(|result| does_track_match_search(track, result)))
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ITunesStoreSong {
    #[serde(rename = "artistViewUrl")]
    pub artist_apple_music_url: Option<String>,
    pub artist_name: String,
    
    #[serde(rename = "trackCensoredName")]
    pub name_censored: String,
    #[serde(rename = "trackName")]
    pub name: String,

    #[serde(rename = "artworkUrl100")]
    pub artwork_preview_url: String,

    #[serde(rename = "trackViewUrl")]
    pub apple_music_url: String,

    #[serde(rename = "collectionCensoredName")]
    pub collection_name_censored: String,
    pub collection_name: String,
}
impl ITunesStoreSong {
    pub fn get_artwork_url_at_resolution(&self, resolution: u16) -> String {
        let replacement = format!("{}x{}", resolution, resolution);
        self.artwork_preview_url.replace("100x100", &replacement)
    } 
}
