#![allow(dead_code)]

use serde::{de::Error as _, Deserialize};

const ITUNES_API_BASE_URL: &str = "https://itunes.apple.com";

fn deserialize_results<T>(response: &str) -> Result<Vec<T>, serde_json::Error> where T: for<'de> Deserialize<'de> {
    pub fn with_deserializer<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
    where
        D: serde::Deserializer<'de>,
        T: serde::Deserialize<'de>,
    {
        use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

        struct ResultsVisitor<T> {
            marker: std::marker::PhantomData<fn() -> Vec<T>>,
        }

        impl<'de, T> Visitor<'de> for ResultsVisitor<T> where T: Deserialize<'de>,{
            type Value = Vec<T>;

            fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
                formatter.write_str("a map with 'resultCount' and 'results'")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Vec<T>, M::Error> where M: MapAccess<'de> {
                let mut capacity = 0;
                let mut results = None;

                while let Some(key) = map.next_key::<&str>()? {
                    match key {
                        "resultCount" => capacity = map.next_value::<usize>()?,
                        "results" => {
                            results = Some(map.next_value_seed(CapacityHintedSequenceSeed {
                                capacity,
                                marker: std::marker::PhantomData,
                            })?);
                        }
                        _ => {
                            let _ = map.next_value::<de::IgnoredAny>()?;
                        }
                    }
                }

                let results = results.ok_or_else(|| de::Error::missing_field("results"))?;

                if capacity != results.len() {
                    return Err(de::Error::invalid_length(results.len(), &format!("expected {capacity}").as_str()));
                }

                Ok(results)
            }
        }

        struct CapacityHintedSequenceSeed<T> {
            capacity: usize,
            marker: std::marker::PhantomData<T>,
        }

        impl<'de, T> de::DeserializeSeed<'de> for CapacityHintedSequenceSeed<T> where T: Deserialize<'de> {
            type Value = Vec<T>;

            fn deserialize<D>(self, deserializer: D) -> Result<Vec<T>, D::Error> where D: Deserializer<'de> {
                struct SequenceVisitor<T> {
                    capacity: usize,
                    marker: std::marker::PhantomData<T>,
                }

                impl<'de, T> Visitor<'de> for SequenceVisitor<T> where T: Deserialize<'de> {
                    type Value = Vec<T>;

                    fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
                        formatter.write_str("a sequence of results")
                    }

                    fn visit_seq<A>(self, mut seq: A) -> Result<Vec<T>, A::Error> where A: SeqAccess<'de> {
                        let mut vec = Vec::with_capacity(self.capacity);

                        while let Some(value) = seq.next_element()? {
                            vec.push(value);
                        }

                        Ok(vec)
                    }
                }

                deserializer.deserialize_seq(SequenceVisitor {
                    capacity: self.capacity,
                    marker: self.marker,
                })
            }
        }

        deserializer.deserialize_map(ResultsVisitor {
            marker: std::marker::PhantomData,
        })
    }

    let mut deserializer = serde_json::Deserializer::from_str(response);
    let results: Vec<T> = with_deserializer(&mut deserializer)?;
    Ok(results)
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
        mzstatic::image::ParseError<'_>,
    > {
        mzstatic::image::MzStaticImage::parse(&self.artwork_preview_url)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("HTTP error: {0}")]
    NetworkFailed(#[from] reqwest::Error),
    #[error("JSON error: {0}")]
    DeserializationFailed(#[from] serde_json::Error),
}

pub struct Client {
    reqwest: reqwest::Client,
}
impl Client {
    pub fn new(reqwest_client: reqwest::Client) -> Self {
        Self {
            reqwest: reqwest_client
        }
    } 

    async fn lookup<T>(&self, id: u32, entity: &str) -> Result<Option<T>, Error> where T: for<'de> Deserialize<'de> {
        let url = format!("{ITUNES_API_BASE_URL}/lookup?id={id}&entity={entity}");
        let response = self.reqwest.get(&url).send().await?;
        let json = response.text().await?;
        Ok(deserialize_results::<T>(&json)?.into_iter().next())
    }

    pub async fn lookup_artist(&self, id: u32) -> Result<Option<Artist>, Error> {
        self.lookup(id, "musicArtist").await
    }

    pub async fn search_songs(&self, query: &str, limit: usize) -> Result<Vec<Track>, Error> {
        let mut url = reqwest::Url::parse(format!("{ITUNES_API_BASE_URL}/search").as_str()).unwrap();
        url.query_pairs_mut()
            .append_pair("term", query)
            .append_pair("entity", "song")
            .append_pair("limit", &limit.to_string());

        let res = self.reqwest.get(url).send().await?;
        let text = res.text().await.map_err(|_| Error::DeserializationFailed(serde_json::Error::custom("could not decode response")))?;
        Ok(deserialize_results::<Track>(&text)?)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires network connection"]
    async fn test_lookup_artist() {
        let client = Client::new(reqwest::Client::new());
        let artist = client.lookup_artist(909253).await.unwrap();
        assert!(artist.is_some());
    }
}
