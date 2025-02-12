use std::sync::Arc;
use maybe_owned_string::MaybeOwnedStringDeserializeToOwned;

use super::StatusBackend;

const FOUR_MINUTES: core::time::Duration = core::time::Duration::from_secs(4 * 60);

use brainz::music::request_client::ProgramInfo;

type S = MaybeOwnedStringDeserializeToOwned<'static>;
type P = ProgramInfo<S>;

pub static DEFAULT_PROGRAM_INFO: P = ProgramInfo {
    contact: MaybeOwnedStringDeserializeToOwned::borrowed(crate::util::REPOSITORY_URL),
    name: MaybeOwnedStringDeserializeToOwned::borrowed(clap::crate_name!()),
    version: Some(MaybeOwnedStringDeserializeToOwned::borrowed(clap::crate_version!())),
};

fn get_default_program_info() -> P {
    DEFAULT_PROGRAM_INFO.clone()
}

fn is_default_program_info(info: &ProgramInfo<MaybeOwnedStringDeserializeToOwned<'_>>) -> bool {
    info == &DEFAULT_PROGRAM_INFO
}


#[derive(serde::Serialize, serde::Deserialize)]


pub struct Config {
    pub enabled: bool,
    #[serde(
        default = "get_default_program_info",
        skip_serializing_if = "is_default_program_info"
    )]
    pub program_info: ProgramInfo<S>,
    pub user_token: Option<brainz::listen::v1::UserToken>,
}


pub struct ListenBrainz {
    client: Arc<brainz::listen::v1::Client<S>>,
}
impl core::fmt::Debug for ListenBrainz {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ListenBrainz").finish()
    }
}
impl ListenBrainz {
    #[tracing::instrument]
    pub fn new(program_info: ProgramInfo<MaybeOwnedStringDeserializeToOwned<'static>>, token: brainz::listen::v1::UserToken) -> Self {
        Self { client: Arc::new(brainz::listen::v1::Client::new(program_info, Some(token))) }
    }

    fn basic_track_metadata(track: &osa_apple_music::track::Track) -> brainz::listen::v1::submit_listens::BasicTrackMetadata<'_> {
        brainz::listen::v1::submit_listens::BasicTrackMetadata {
            artist: &track.artist.as_ref().map(String::as_str).unwrap_or("Unknown Artist"),
            track: &track.name,
            release: track.album.name.as_ref().map(String::as_str)
        }
    }

    fn additional_info<'a>(track: &'a osa_apple_music::track::Track, app: &'a osa_apple_music::application::ApplicationData, program: &'a brainz::music::request_client::ProgramInfo<S>) -> brainz::listen::v1::submit_listens::additional_info::AdditionalInfo<'a> {
        use brainz::listen::v1::submit_listens::additional_info::*;
        AdditionalInfo {
            duration: track.duration.map(|d| core::time::Duration::from_secs_f32(d)),
            track_number: track.track_number.map(|n| n.get() as u32),
            submission_client: Some(program),
            music_service: Some(MusicService::Domain("music.apple.com")),
            media_player: Some(MediaPlayer {
                name: "Apple Music",
                version: Some(&app.version)
            }),
            ..Default::default()
        }
    }
}
#[async_trait::async_trait]
impl StatusBackend for ListenBrainz {
    #[tracing::instrument(level = "debug")]   
    async fn record_as_listened(&self, track: Arc<osa_apple_music::track::Track>, app: Arc<osa_apple_music::application::ApplicationData>) {
        // TODO: catch net error or add to queue. ideally queue persist offline
        if let Err(error) = self.client.submit_playing_now(
            Self::basic_track_metadata(&track),
            Some(Self::additional_info(&track, &app, self.client.get_program_info()))
        ).await {
            tracing::error!(?error, "listenbrainz mark-listened failure")
        }
    }

    /// - <https://listenbrainz.readthedocs.io/en/latest/users/api/core.html#post--1-submit-listens>
    async fn check_eligibility(&self, track: Arc<osa_apple_music::track::Track>, time_listened: &core::time::Duration) -> bool {
        if let Some(duration) = track.duration {
            let length = core::time::Duration::from_secs_f32(duration);
            time_listened >= &FOUR_MINUTES ||
            time_listened.as_secs_f64() >= (length.as_secs_f64() / 2.)
        } else { false }
    }

    #[tracing::instrument(level = "debug")]
    async fn set_now_listening(&mut self, track: Arc<osa_apple_music::track::Track>, app: Arc<osa_apple_music::application::ApplicationData>, _: Arc<crate::data_fetching::AdditionalTrackData>) {
        if let Err(error) = self.client.submit_playing_now(
            Self::basic_track_metadata(&track),
            Some(Self::additional_info(&track, &app, self.client.get_program_info()))
        ).await {
            tracing::error!(?error, "listenbrainz now-listening failure")
        }
    }
}

