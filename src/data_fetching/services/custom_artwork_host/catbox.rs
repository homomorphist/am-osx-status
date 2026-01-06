use crate::subscribers::DispatchableTrack;

/// Free file host, <https://catbox.moe/>.
#[derive(Debug, Default)]
pub struct CatboxHost;
#[async_trait::async_trait]
impl super::CustomArtworkHost for CatboxHost {
    async fn new((): &<Self as super::CustomArtworkHostMetadata>::Config) -> Self where Self: Sized + super::CustomArtworkHostMetadata {
        Self
    }
    
    async fn upload(&mut self, pool: &sqlx::SqlitePool, _: &DispatchableTrack, path: &str) -> Result<crate::store::entities::CustomArtworkUrl, super::UploadError> {
        const EXPIRES_IN_HOURS: u16 = 24 * 31 * 6; // i think we can trust they'll stay online 6 months :]

        let url = ::catbox::file::from_file(path, None).await.map_err(|error| {
            tracing::error!(?error, ?path, "catbox upload error");
            super::UploadError::UnknownError
        })?;

        if url.contains("Internal Server Error") {
            tracing::debug!(?url, ?path); // it dumps an entire html page for some godforsaken reason
            tracing::error!(?path, "catbox upload returned internal server error");
            return Err(super::UploadError::UnknownError);
        }

        let expires_at = chrono::Utc::now() + chrono::Duration::hours(i64::from(EXPIRES_IN_HOURS));
        Ok(crate::store::entities::CustomArtworkUrl::new(pool, Some(expires_at), path, &url).await?)
    }
}
impl super::CustomArtworkHostMetadata for CatboxHost {
    const IDENTITY: super::HostIdentity = super::HostIdentity::Catbox;
    type Config = ();
}

pub use CatboxHost as Host;
