use crate::subscribers::DispatchableTrack;
/// Free temporary file host, <https://litterbox.catbox.moe/>.
#[derive(Debug, Default)]
pub struct LitterboxHost;
#[async_trait::async_trait]
impl super::CustomArtworkHost for LitterboxHost {
    async fn new((): &<Self as super::CustomArtworkHostMetadata>::Config) -> Self where Self: Sized + super::CustomArtworkHostMetadata {
        Self
    }
    
    async fn upload(&mut self, client: &reqwest::Client, pool: &sqlx::SqlitePool, _: &DispatchableTrack, path: &std::path::Path) -> Result<crate::store::entities::CustomArtworkUrl, super::UploadError> {
        use reqwest::{Body, multipart::{Form, Part}};

        const EXPIRES_IN: ExpiresAfter = ExpiresAfter::OneDay;

        let stream = Body::wrap_stream(tokio_util::io::ReaderStream::new(tokio::fs::File::open(path).await?));
        let name = path.file_name().expect("file has no name").to_string_lossy().to_string();
        let form = Form::new()
            .text("reqtype", "fileupload")
            .text("time", EXPIRES_IN.as_hours_str())
            .part("fileToUpload", Part::stream(stream).file_name(name));

        let url = client
            .post("https://litterbox.catbox.moe/resources/internals/api.php")
            .multipart(form)
            .send().await?.error_for_status()?
            .text().await?;

        // Not sure if this is needed now that we have `error_for_status`, but better safe than sorry
        if url.contains("Internal Server Error") {
            tracing::debug!(?url, ?path); // it dumps an entire html page for some godforsaken reason
            tracing::error!(?path, "Litterbox upload returned internal server error");
            return Err(super::UploadError::Unknown);
        }

        let expires_at = chrono::Utc::now() + EXPIRES_IN.as_duration();
        Ok(crate::store::entities::CustomArtworkUrl::new(pool, Some(expires_at), path, &url).await?)
    }
}
impl super::CustomArtworkHostMetadata for LitterboxHost {
    const IDENTITY: super::HostIdentity = super::HostIdentity::Litterbox;
    type Config = ();
}

macro_rules! define_expire_times {
    ($( $name:ident = $value:expr ),* $(,)?) => {
        #[allow(unused)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum ExpiresAfter {
            $( $name, )*
        }
        impl ExpiresAfter {
            pub const fn as_hours_str(self) -> &'static str {
                match self {
                    $( ExpiresAfter::$name => concat!(stringify!($value), "h"), )*
                }
            }
            pub const fn as_duration(self) -> core::time::Duration {
                match self {
                    $( ExpiresAfter::$name => core::time::Duration::from_hours($value), )*
                }
            }
        }
    }
}

define_expire_times! {
    OneHour = 1,
    TwelveHours = 12,
    OneDay = 24,
    ThreeDays = 72,
}

pub use LitterboxHost as Host;
