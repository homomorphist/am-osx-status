use std::collections::HashMap;

#[derive(Debug)]
struct Entry {
    url: String,
    expires_at: chrono::DateTime<chrono::Utc>
}

#[derive(Debug, Default)]
pub struct CatboxHost(HashMap</* album key */ String, Entry>);
#[async_trait::async_trait]
impl super::CustomArtworkHost for CatboxHost {
    async fn get_for_track(&self, track: &apple_music::Track) -> Result<Option<String>, super::RetrievalError> {
        // do i really gotta clone here ??
        if let Some(entry) = self.0.get(&Self::key_for_track(track)) {
            const EXTERNAL_ACCESS_DELAY: chrono::Duration = chrono::Duration::seconds(5);
            let did_expire =  entry.expires_at < chrono::Utc::now() + EXTERNAL_ACCESS_DELAY; // or will expire in next 5 seconds
            if did_expire { Ok(None) } else { Ok(Some(entry.url.clone())) }
        } else { Ok(None) }
    }
    
    async fn upload_for_track(&mut self, track: &apple_music::Track, path: &str) -> Result<String, super::UploadError> {
        const EXPIRES_IN_HOURS: u8 = 1;
        let url = ::catbox::litter::upload(path, EXPIRES_IN_HOURS).await.map_err(|error| {
            tracing::error!(error);
            super::UploadError::UnknownError
        })?;
        let expires_at = chrono::Utc::now() + chrono::Duration::hours(EXPIRES_IN_HOURS as i64);
        self.0.insert(Self::key_for_track(track), Entry { url: url.clone(), expires_at });
        Ok(url)
    }
}
impl CatboxHost {
    fn key_for_track(track: &apple_music::Track) -> String {
        // no consistent access to album persistent id (musicdb support may be disabled); merge unique-ish details
        format!("{}:{}", track.artist, track.album)
    }

    pub fn new() -> Self {
        Self::default()
    }
}
