use serde::{de::Error, Deserialize};

const ITUNES_API_BASE_URL: &str = "https://itunes.apple.com";

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Results<T> {
    #[allow(unused)] // TODO: Use for allocation?
    pub result_count: i32,
    pub results: Vec<T>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Artist {
    #[serde(rename = "artistName")]
    pub name: String,
    #[serde(rename = "artistId")]
    pub id: u32,
    #[serde(rename = "primaryGenreName")]
    pub genre: String,
    #[serde(rename = "primaryGenreId")]
    pub genre_id: u32,
    #[serde(rename = "amgArtistId")]
    pub amg_id: Option<core::num::NonZeroU32>,
    #[serde(rename = "artistLinkUrl")]
    pub link: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Track {
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
impl Track {
    pub fn artwork_mzstatic(&self) -> Result<
        mzstatic::image::MzStaticImage<'_>,
        mzstatic::image::ParseError,
    > {
        mzstatic::image::MzStaticImage::parse(&self.artwork_preview_url)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum RequestError {
    #[error("HTTP error: {0}")]
    NetworkFailed(#[from] reqwest::Error),
    #[error("JSON error: {0}")]
    DeserializationFailed(#[from] serde_json::Error),
}

// TODO: reuse client.
pub async fn lookup_artist(id: u32) -> Result<Option<Artist>, RequestError> {
    let url = format!("{ITUNES_API_BASE_URL}/lookup?id={id}");
    let response = reqwest::get(&url).await?.text().await?;
    let response = serde_json::from_str::<Results<Artist>>(&response)?;
    Ok(response.results.into_iter().next())
}

pub async fn search_songs(query: &str, limit: usize) -> Result<Vec<Track>, RequestError> {
    #[allow(unused)]
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ITunesSongSearchOutcome {
        result_count: i32,
        results: Vec<Track>,
    }

    let mut url = reqwest::Url::parse(format!("{ITUNES_API_BASE_URL}/search").as_str()).unwrap();
    url.query_pairs_mut()
        .append_pair("term", query)
        .append_pair("entity", "song")
        .append_pair("limit", &limit.to_string());

    let res = reqwest::get(url).await.map_err(RequestError::NetworkFailed)?;
    let text = res.text().await.map_err(|_| RequestError::DeserializationFailed(serde_json::Error::custom("could not decode response")))?;
    serde_json::from_str::<ITunesSongSearchOutcome>(&text)
        .map(|outcome: ITunesSongSearchOutcome| outcome.results)
        .map_err(RequestError::DeserializationFailed)
}
