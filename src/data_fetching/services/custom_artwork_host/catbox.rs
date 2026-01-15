use crate::subscribers::DispatchableTrack;

/// Free file host, <https://catbox.moe/>.
#[derive(Debug, Default)]
pub struct CatboxHost;
#[async_trait::async_trait]
impl super::CustomArtworkHost for CatboxHost {
    async fn new((): &<Self as super::CustomArtworkHostMetadata>::Config) -> Self where Self: Sized + super::CustomArtworkHostMetadata {
        Self
    }
    
    async fn upload(&mut self, client: &reqwest::Client, pool: &sqlx::SqlitePool, _: &DispatchableTrack, path: &std::path::Path) -> Result<crate::store::entities::CustomArtworkUrl, super::UploadError> {
        const TIME_UNTIL_URL_INVALIDATED: core::time::Duration = core::time::Duration::from_hours(24 * 31 * 6); // 6 months — i think we can trust they'll stay online until then :]

        use reqwest::{Body, multipart::{Form, Part}};

        let stream = Body::wrap_stream(tokio_util::io::ReaderStream::new(tokio::fs::File::open(path).await?));
        let name = path.file_name().expect("file has no name").to_string_lossy().to_string();
        let form = Form::new()
            .text("reqtype", "fileupload")
            .text("userhash", "")
            .part("fileToUpload", Part::stream(stream).file_name(name));

        let url = client
            .post("https://catbox.moe/user/api.php")
            .multipart(form)
            .send().await?.error_for_status()?
            .text().await?;

        // Not sure if this is needed now that we have `error_for_status`, but better safe than sorry
        if url.contains("Internal Server Error") {
            tracing::debug!(?url, ?path); // it dumps an entire html page for some godforsaken reason
            tracing::error!(?path, "catbox upload returned internal server error");
            return Err(super::UploadError::Unknown);
        }

        let expires_at = chrono::Utc::now() + TIME_UNTIL_URL_INVALIDATED;
        Ok(crate::store::entities::CustomArtworkUrl::new(pool, Some(expires_at), path, &url).await?)
    }
}
impl super::CustomArtworkHostMetadata for CatboxHost {
    const IDENTITY: super::HostIdentity = super::HostIdentity::Catbox;
    type Config = ();
}

pub use CatboxHost as Host;
