use maybe_owned_string::MaybeOwnedString;

pub mod catbox;

#[derive(thiserror::Error, Debug)]
pub enum UploadError {
    #[error("an unknown error occurred while uploading the custom track artwork")]
    UnknownError,
}

#[derive(thiserror::Error, Debug)]
pub enum RetrievalError {
    #[error("an unknown error occurred while retrieving the custom track artwork url")]
    UnknownError,
}


#[derive(thiserror::Error, Debug)]
pub enum CustomArtworkHostError {
    #[error("{0}")]
    UploadError(#[from] UploadError),
    #[error("{0}")]
    RetrievalError(#[from] RetrievalError)
}

#[async_trait::async_trait]
pub trait CustomArtworkHost: core::fmt::Debug + Send {
    async fn get_for_track(&self, track: &apple_music::Track) -> Result<Option<String>, RetrievalError>;
    async fn upload_for_track(&mut self, track: &apple_music::Track, path: &str) -> Result<String, UploadError>;
    async fn for_track(&mut self, track: &apple_music::Track, path: &str) -> Result<String, CustomArtworkHostError> {
        match self.get_for_track(track).await.map_err(CustomArtworkHostError::RetrievalError)? {
            Some(url) => Ok(url),
            None => self.upload_for_track(track, path).await.map_err(CustomArtworkHostError::UploadError)
        }
    }
}
