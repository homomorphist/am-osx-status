use std::collections::HashMap;
use crate::{data_fetching::services, subscribers::DispatchableTrack};

#[derive(Debug, Default)]
pub struct CatboxHost;
#[async_trait::async_trait]
impl super::CustomArtworkHost for CatboxHost {
    async fn new(config: &<Self as super::CustomArtworkHostMetadata>::Config) -> Self where Self: Sized + super::CustomArtworkHostMetadata {
        Self
    }
    
    async fn upload(&mut self, pool: &sqlx::SqlitePool, track: &DispatchableTrack, path: &str) -> Result<crate::store::entities::CustomArtworkUrl, super::UploadError> {
        const EXPIRES_IN_HOURS: u8 = 1;
        let url = ::catbox::litter::upload(path, EXPIRES_IN_HOURS).await.map_err(|error| {
            tracing::error!(?error, ?path, "catbox upload error");
            super::UploadError::UnknownError
        })?;
        let expires_at = chrono::Utc::now() + chrono::Duration::hours(EXPIRES_IN_HOURS as i64);
        Ok(crate::store::entities::CustomArtworkUrl::new(pool, Some(expires_at), path, &url).await?)
    }
}
impl super::CustomArtworkHostMetadata for CatboxHost {
    const IDENTITY: super::HostIdentity = super::HostIdentity::Catbox;
    type Config = ();
}
impl CatboxHost {
    fn key_for_track(track: &DispatchableTrack) -> String {
        // no consistent access to album persistent id (musicdb support may be disabled); merge unique-ish details
        format!("{}:{}",
            track.artist.as_deref().unwrap_or("Unknown Artist"),
            track.album.as_deref().unwrap_or("Unknown Album")
        )
    }
}

pub use CatboxHost as Host;
