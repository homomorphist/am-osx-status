use std::{fmt::Debug, sync::Arc};
use chrono::TimeDelta;

use super::{StatusBackend, TimeDeltaExtension as _};

const FOUR_MINUTES: TimeDelta = TimeDelta::new(4 * 60, 0).unwrap();
const THIRTY_SECONDS: TimeDelta = TimeDelta::new(30, 0).unwrap();

use std::sync::LazyLock;
use lastfm::auth::ClientIdentity;

pub static DEFAULT_CLIENT_IDENTITY: LazyLock<ClientIdentity> = LazyLock::new(|| {
    ClientIdentity::new(
        concat!(
            clap::crate_name!(), "/",
            clap::crate_version!()
        ).to_owned(),
        "d591a37a79ec4c3d4efe55379029b5b3",
        "20a069921b30039bd2601d955e3bce46"
    ).expect("bad built-in client identity")
});

fn get_default_client_identity() -> ClientIdentity {
    DEFAULT_CLIENT_IDENTITY.clone()
}

fn is_default_client_identity(identity: &ClientIdentity) -> bool {
    identity == &*DEFAULT_CLIENT_IDENTITY
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Config {
    pub enabled: bool,
    #[serde(
        default = "get_default_client_identity",
        skip_serializing_if = "is_default_client_identity"
    )]
    pub identity: ClientIdentity,
    pub session_key: Option<lastfm::auth::SessionKey>
}


pub struct LastFM {
    client: Arc<::lastfm::Client<::lastfm::auth::state::Authorized>>
}
impl Debug for LastFM {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LastFM").finish()
    }
}
impl LastFM {
    pub fn new(identity: ClientIdentity, session_key: lastfm::auth::SessionKey) -> Self {
        let client = lastfm::Client::authorized(identity, session_key);
        Self { client: Arc::new(client) }
    }

    /// Returns `None` if the track is missing required data (the artist or track name).
    fn track_to_heard(track: &osa_apple_music::track::Track) -> Option<lastfm::scrobble::HeardTrackInfo<'_>> {
        Some(lastfm::scrobble::HeardTrackInfo {
            artist: track.artist.as_ref().map(|s| s.split(" & ").next().unwrap())?,
            track: &track.name,
            album: track.album.name.as_deref(),
            album_artist: if track.album.artist.as_ref().is_some_and(|aa| Some(aa) != track.artist.as_ref()) { Some(track.album.artist.as_ref().unwrap()) } else { None },
            duration_in_seconds: track.duration.map(|d| d as u32),
            track_number: track.track_number.map(|n| n.get() as u32),
            mbid: None
        })
    }
}
#[async_trait::async_trait]
impl StatusBackend for LastFM {
    #[tracing::instrument(skip(self, context), level = "debug")]
    async fn record_as_listened(&self, context: super::BackendContext<()>) {
        if let Some(info) = Self::track_to_heard(context.track.as_ref()) {
            if let Err(error) = self.client.scrobble(&[lastfm::scrobble::Scrobble {
                chosen_by_user: None,
                timestamp: chrono::Utc::now(),
                info
            }]).await {
                tracing::error!(?error, "last.fm mark-listened failure")
            }
        } else {
            tracing::warn!("scrobble skipped; track is missing required data (artist name)")
        }
    }

    /// - <https://www.last.fm/api/scrobbling#scrobble-requests>
    async fn check_eligibility(&self, context: super::BackendContext<()>) -> bool {
        if let Some(duration) = context.track.duration {
            let length = TimeDelta::from_secs_f32(duration);
            let time_listened = context.listened.lock().await.total_heard();
            if length < THIRTY_SECONDS { return false };
            time_listened >= FOUR_MINUTES ||
            time_listened.as_secs_f32() >= (length.as_secs_f32() / 2.)
        } else { false }
    }

    #[tracing::instrument(skip(self, context), level = "debug")]
    async fn set_now_listening(&mut self, context: super::BackendContext<crate::data_fetching::AdditionalTrackData>) {
        if let Some(info) = Self::track_to_heard(context.track.as_ref()) {
            if let Err(error) = self.client.set_now_listening(&info).await {
                tracing::error!(?error, "last.fm now-listening dispatch failure")
            }
        } else {
            tracing::warn!("last.fm now-listening dispatch skipped; track is missing required data (artist name)")
        }
    }
}
