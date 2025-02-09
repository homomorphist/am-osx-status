use std::{fmt::Debug, sync::Arc, time::Duration};
use apple_music::Track;

use super::StatusBackend;

const FOUR_MINUTES: Duration = Duration::from_secs(4 * 60);
const THIRTY_SECONDS: Duration = Duration::from_secs(30);

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

    fn track_to_heard(track: &Track) -> lastfm::scrobble::HeardTrackInfo<'_> {
        lastfm::scrobble::HeardTrackInfo {
            artist: track.artist.split(" & ").next().unwrap(),
            track: &track.name,
            album: Some(&track.album),
            album_artist: if track.artist != track.album_artist { Some(&track.album_artist) } else { None },
            duration_in_seconds: Some(track.duration as u32),
            track_number: Some(track.track_number as u32),
            mbid: None
        }
    }
}
#[async_trait::async_trait]
impl StatusBackend for LastFM {
    #[tracing::instrument(level = "debug")]
    async fn record_as_listened(&self, track: Arc<Track>, _: Arc<apple_music::ApplicationData>) {
        if let Err(error) = self.client.scrobble(&[lastfm::scrobble::Scrobble {
            chosen_by_user: None,
            timestamp: chrono::Utc::now(),
            info: Self::track_to_heard(track.as_ref())
        }]).await {
            tracing::error!(?error, "last.fm mark-listened failure")
        }
    }

    /// - <https://www.last.fm/api/scrobbling#scrobble-requests>
    async fn check_eligibility(&self, track: Arc<Track>, listened: Arc<tokio::sync::Mutex<super::Listened>>) -> bool {
        let length = Duration::from_secs_f64(track.duration);
        let time_listened = listened.lock().await.total_heard();
        if length < THIRTY_SECONDS { return false };
        time_listened >= FOUR_MINUTES ||
        time_listened.as_secs_f64() >= (length.as_secs_f64() / 2.)
    }

    #[tracing::instrument(level = "debug")]
    async fn set_now_listening(&mut self, track: Arc<Track>, _: Arc<apple_music::ApplicationData>, _: Arc<crate::data_fetching::AdditionalTrackData>) {
        if let Err(error) = self.client.set_now_listening(&Self::track_to_heard(track.as_ref())).await {
            tracing::error!(?error, "last.fm now-listening dispatch failure")
        }
    }
}
