use itunes_api::Client;
use serde::Deserialize;

use unicode_normalization::UnicodeNormalization;

fn normalize(string: &str) -> String {
    string.trim().nfkc().collect::<String>().to_lowercase()
}

// TODO: Rank with numeric. With Levenshtein; after removing parentheses, ignoring album, stuff like that.
fn does_track_match_search(track: &crate::subscribers::DispatchableTrack, found: &itunes_api::Track) -> bool {
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

pub async fn find_track(track: &crate::subscribers::DispatchableTrack) -> Result<Option<itunes_api::Track>, itunes_api::Error> {
    let query: String = format!("{} {}", track.artist.clone().unwrap_or_default(), track.name);
    let client = Client::new(reqwest::Client::new()); // TODO: use a shared client.
    let songs = client.search_songs(&query, 10).await?;
    if songs.len() == 1 { return Ok(songs.into_iter().next()) }
    Ok(songs.into_iter().find(|result| does_track_match_search(track, result)))
}
