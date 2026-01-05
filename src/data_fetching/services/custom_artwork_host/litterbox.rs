use crate::{data_fetching::services, subscribers::DispatchableTrack};

/// Free temporary file host, <https://litterbox.catbox.moe/>.
#[derive(Debug, Default)]
pub struct LitterboxHost;
#[async_trait::async_trait]
impl super::CustomArtworkHost for LitterboxHost {
    async fn new(config: &<Self as super::CustomArtworkHostMetadata>::Config) -> Self where Self: Sized + super::CustomArtworkHostMetadata {
        Self
    }
    
    async fn upload(&mut self, pool: &sqlx::SqlitePool, track: &DispatchableTrack, path: &str) -> Result<crate::store::entities::CustomArtworkUrl, super::UploadError> {
        const EXPIRES_IN_HOURS: u8 = 12;

        let url = ::catbox::litter::upload(path, EXPIRES_IN_HOURS).await.map_err(|error| {
            tracing::error!(?error, ?path, "Litterbox upload error");
            super::UploadError::UnknownError
        })?;

        if url.contains("Internal Server Error") {
            tracing::debug!(?url, ?path); // it dumps an entire html page for some godforsaken reason
            tracing::error!(?path, "Litterbox upload returned internal server error");
            return Err(super::UploadError::UnknownError);
        }

        let expires_at = chrono::Utc::now() + chrono::Duration::hours(EXPIRES_IN_HOURS as i64);
        Ok(crate::store::entities::CustomArtworkUrl::new(pool, Some(expires_at), path, &url).await?)
    }
}
impl super::CustomArtworkHostMetadata for LitterboxHost {
    const IDENTITY: super::HostIdentity = super::HostIdentity::Litterbox;
    type Config = ();
}

pub use LitterboxHost as Host;
