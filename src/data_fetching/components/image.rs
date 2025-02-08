use crate::data_fetching::services::itunes::ITunesStoreSong;

#[derive(Default, Debug)]
pub struct TrackImageUrlPack {
    pub artist: Option<String>,
    pub track: Option<String>
}
impl TrackImageUrlPack {
    pub fn none() -> Self {
        Self::default()
    }

    pub async fn from_itunes(song: ITunesStoreSong) -> TrackImageUrlPack {
        TrackImageUrlPack {
            track: Some(song.get_artwork_url_at_resolution(500)),
            artist: match song.artist_apple_music_url {
                Some(artist_url) => super::super::services::apple_music::scrape_artist_image(&artist_url, 100).await.unwrap_or_default(),
                None => None
            }
        }
    }
}

