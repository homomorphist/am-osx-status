use itunes_api::Client;
use unicode_normalization::UnicodeNormalization;

fn normalize(string: &str) -> String {
    string.trim().nfkc().collect::<String>().to_lowercase()
}

pub struct Query<'a> {
    pub title: &'a str,
    pub album: Option<&'a str>,
    pub artist: Option<&'a str>,
}

// TODO: Rank with numeric. With Levenshtein; after removing parentheses, ignoring album, stuff like that.
fn does_track_match_search(track: &Query, found: &itunes_api::Track) -> bool {
    let name = normalize(track.title);
    let artist = normalize(track.artist.unwrap_or_default());
    let collection = normalize(track.album.unwrap_or_default());

    (
        normalize(&found.name) == name ||
        normalize(&found.name_censored) == name
    )
        && (normalize(&found.artist_name) == artist)
        && (normalize(&found.collection_name) == collection)
}

pub async fn find_track(query: &Query<'_>) -> Result<Option<itunes_api::Track>, itunes_api::Error> {
    let search = format!("{} {}", query.artist.unwrap_or_default(), query.title);
    let search = search.trim();
    let client = Client::new(reqwest::Client::new()); // TODO: use a shared client.
    let songs = client.search_songs(search, 10).await?;
    if songs.len() == 1 { return Ok(songs.into_iter().next()) }
    Ok(songs.into_iter().find(|result| does_track_match_search(query, result)))
}
