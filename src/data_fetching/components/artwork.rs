#[derive(Debug, Clone)]
pub enum LocatedResource {
    Remote(String),
    Local(std::path::PathBuf),
}
impl LocatedResource {
    pub async fn into_uploaded(self, host: &ArtworkManager, track: &crate::subscribers::DispatchableTrack) -> Option<String> {
        match self {
            Self::Remote(url) => Some(url),
            Self::Local(path) => host.hosted(&path, track).await.map(|v| v.url),
        }
    }
    #[allow(dead_code, reason = "used only by certain featured-gated backends")]
    pub const fn as_url(&self) -> Option<&str> {
        match self {
            Self::Remote(url) => Some(url.as_str()),
            Self::Local(_) => None
        }
    }
    #[expect(dead_code, reason = "might be useful later")]
    pub fn as_path(&self) -> Option<&std::path::Path> {
        match self {
            Self::Remote(_) => None,
            Self::Local(path) => Some(path.as_path()),
        }
    }
}
impl From<&mzstatic::image::MzStaticImage<'_>> for LocatedResource {
    fn from(mzstatic: &mzstatic::image::MzStaticImage) -> Self {
        Self::Remote(mzstatic.to_string())
    }
}

use crate::data_fetching::services::custom_artwork_host;
use crate::store::entities::CustomArtworkUrl;

#[derive(Debug)]
pub struct ArtworkManager {
    host_order: custom_artwork_host::OrderedHostList,
    hosts: custom_artwork_host::Hosts,
    network_client: reqwest::Client,
}
impl ArtworkManager {
    pub async fn new(host_configurations: &custom_artwork_host::HostConfigurations) -> Self {
        Self {
            hosts: custom_artwork_host::Hosts::new(host_configurations).await,
            host_order: host_configurations.order.clone(),
            network_client: reqwest::ClientBuilder::new()
                .user_agent(concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")))
                .build()
                .expect("failed to build reqwest client"),
        }
    }

    pub async fn hosted(&self, file_path: &std::path::Path, track: &crate::subscribers::DispatchableTrack) -> Option<CustomArtworkUrl> {
        let pool = crate::store::DB_POOL.get().await.expect("failed to get pool");

        if let Some(existing) = CustomArtworkUrl::get_by_source_path_in_pool(&pool, file_path).await.ok().flatten() {
            if existing.is_expired() {
                tracing::warn!(?file_path, "custom artwork url is expired, re-uploading and performing cleanup");
                if let Err(err) = CustomArtworkUrl::cleanup(&pool).await {
                    tracing::error!(?err, "failed to clean up expired custom artwork urls");
                }
            } else {
                tracing::debug!(?file_path, url = ?existing.url, "custom artwork url already exists, returning existing");
                return Some(existing);
            }
        }   

        for identity in &self.host_order.0 {
            match self.hosts.get(*identity).await?.upload(&self.network_client, &pool, track, file_path).await {
                Ok(url) => return Some(url),
                Err(err) => tracing::warn!(?err, "failed to upload custom artwork")
            }
        }
        if self.host_order.0.is_empty() {
            tracing::warn!("no custom artwork hosts available");
        } else {
            tracing::error!("all custom artwork hosts failed to upload artwork");
        }
        None
    }

    pub async fn get(&self,
        solicitation: &crate::data_fetching::ComponentSolicitation,
        track: &crate::subscribers::DispatchableTrack,
        track_itunes: Option<&itunes_api::Track>,
        #[cfg(feature = "musicdb")] musicdb: Option<&musicdb::MusicDB>,
    ) -> TrackArtworkData {
        use crate::data_fetching::{Component, services::artworkd};

        let mut images = TrackArtworkData::none();

        #[cfg(feature = "musicdb")]
        if solicitation.contains(Component::ArtistImage) && let Some(db) = musicdb {
            let id = musicdb::PersistentId::from(track.persistent_id);
            images.artist = db.tracks().get(&id)
                .and_then(|track| db.get(track.artist_id))
                .and_then(|artist| artist.artwork_url.as_ref())
                .filter(|mz| mz.parameters.effect != Some(mzstatic::image::effect::Effect::SquareFitCircle)) // ugly auto-generated
                .map(LocatedResource::from);
        }

        if solicitation.contains(Component::AlbumImage) {
             if let Some(itunes) = track_itunes.as_ref() {
                images.track = itunes.artwork_mzstatic().map(|mut mzstatic|{
                    use mzstatic::image::quality::Quality;
                    mzstatic.parameters.quality = Some(Quality::new(500).unwrap());
                    LocatedResource::from(&mzstatic)
                }).ok();
            }

            #[cfg(feature = "musicdb")]
            if images.track.is_none() && let Some(db) = musicdb {
                let id = musicdb::PersistentId::from(track.persistent_id);
                images.track = db.tracks().get(&id)
                    .and_then(|track| track.artwork.clone())
                    .map(|mut mz| {
                        if mz.subdomain.starts_with('a') {
                            mz.subdomain = "is1-ssl".into();
                            mz.prefix = Some(mzstatic::image::Prefix::ImageThumbnail);
                            mz.asset_token = mz.asset_token.replacen("4/", "v4/", 1).into();
                        }
                        LocatedResource::from(&mz)
                    });
            }

            if images.track.is_none() {
                let artwork = match artworkd::get_artwork(track.persistent_id.signed()).await {
                    Ok(artwork) => artwork,
                    Err(err) => {
                        tracing::error!(?err, id = %track.persistent_id, "failed to get artwork");
                        None
                    }
                };

                images.track = match artwork {
                    None => None,
                    Some(artwork) => artwork.into_uploaded(self, track).await.map(LocatedResource::Remote)
                };
            }
        }

        images
    }
}


#[derive(Default, Debug)]
#[allow(dead_code, reason = "used only by certain featured-gated backends")]
pub struct TrackArtworkData<T = LocatedResource> {
    pub artist: Option<T>,
    pub track: Option<T>
}
impl<T> TrackArtworkData<T> {
    pub const fn none() -> Self {
        Self {
            artist: None,
            track: None,
        }
    }

    #[expect(dead_code, reason = "i've got plans")] // TODO: make use of this when musicdb isn't available
    async fn apple_music_web_scrape_artist_image(artist_url: &str, resolution: u16) -> Result<Option<String>, reqwest::Error> {
        const ELEMENT: &str = r#"<meta property="og:image" content=""#;
        let res = reqwest::get(artist_url).await?;
        let text = res.text().await.expect("bad body");
        Ok(text.find(ELEMENT).map(|start| {
            use mzstatic::image::quality::Quality;
            let start = start + ELEMENT.len();
            let end = text[start..].find('"').expect("element did not close") + start;
            let mut url = mzstatic::image::MzStaticImage::parse(&text[start..end]).expect("bad url");
            url.parameters.quality = Some(Quality::new(resolution).unwrap());
            url.to_string()
        }))
    }

    #[expect(dead_code, reason = "i've got plans")]
    pub fn track_image_from_itunes(song: &itunes_api::Track) -> Option<String> {
        song.artwork_mzstatic().map(|mut mzstatic|{
            use mzstatic::image::quality::Quality;
            mzstatic.parameters.quality = Some(Quality::new(500).unwrap());
            mzstatic.to_string()
        }).ok()
    }
}
impl TrackArtworkData<LocatedResource> {
    #[allow(dead_code, reason = "used only by certain featured-gated backends")]
    pub fn urls(&self) -> TrackArtworkData<&str> {
        TrackArtworkData {
            artist: self.artist.as_ref().and_then(LocatedResource::as_url),
            track: self.track.as_ref().and_then(LocatedResource::as_url),
        }
    }
}
