use serde::Deserialize;

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

#[derive(thiserror::Error, Debug)]
pub enum LookupError {
    #[error("HTTP error: {0}")]
    NetworkFailed(#[from] reqwest::Error),
    #[error("JSON error: {0}")]
    DeserializationFailed(#[from] serde_json::Error),
}

// TODO: reuse client.
pub async fn lookup_artist(id: u32) -> Result<Option<Artist>, LookupError> {
    let url = format!("https://itunes.apple.com/lookup?id={}", id);
    let response = reqwest::get(&url).await?.text().await?;
    let response = serde_json::from_str::<Results<Artist>>(&response)?;
    Ok(response.results.into_iter().next())
}

