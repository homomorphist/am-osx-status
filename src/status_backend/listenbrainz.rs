use std::sync::Arc;
use maybe_owned_string::MaybeOwnedStringDeserializeToOwned;

use super::{StatusBackend, TimeDeltaExtension as _};

const FOUR_MINUTES: chrono::TimeDelta = chrono::TimeDelta::new(4 * 60, 0).unwrap();

use brainz::{listen::v1::submit_listens::additional_info, music::request_client::ProgramInfo};

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
    pub fn new(program_info: ProgramInfo<MaybeOwnedStringDeserializeToOwned<'static>>, token: brainz::listen::v1::UserToken) -> Self {
        Self { client: Arc::new(brainz::listen::v1::Client::new(program_info, Some(token))) }
    }

    fn basic_track_metadata(track: &osa_apple_music::track::Track) -> Option<brainz::listen::v1::submit_listens::BasicTrackMetadata<'_>> {
        Some(brainz::listen::v1::submit_listens::BasicTrackMetadata {
            artist: track.artist.as_deref()?,
            track: &track.name,
            release: track.album.name.as_deref()
        })
    }

    fn additional_info<'a>(track: &'a osa_apple_music::track::Track, app: &'a osa_apple_music::application::ApplicationData, program: &'a brainz::music::request_client::ProgramInfo<S>) -> brainz::listen::v1::submit_listens::additional_info::AdditionalInfo<'a> {
        use brainz::listen::v1::submit_listens::additional_info::*;
        AdditionalInfo {
            duration: track.duration.map(core::time::Duration::from_secs_f32),
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
    #[tracing::instrument(skip(self, context), level = "debug")]   
    async fn record_as_listened(&self, context: super::BackendContext<()>) {
        if let Some(track_data) = Self::basic_track_metadata(&context.track) {
            let additional_info = Self::additional_info(&context.track, &context.app, self.client.get_program_info());
            // TODO: catch network errors and add to a queue.
            if let Err(error) = self.client.submit_playing_now(track_data, Some(additional_info)).await {
                tracing::error!(?error, "listenbrainz mark-listened failure")
            }
        } else {
            tracing::warn!("listenbrainz mark-listened dispatch skipped; track is missing required data (artist name)")
        }
    }

    /// - <https://listenbrainz.readthedocs.io/en/latest/users/api/core.html#post--1-submit-listens>
    async fn check_eligibility(&self, context: super::BackendContext<()>) -> bool {
        if let Some(duration) = context.track.duration {
            let length = core::time::Duration::from_secs_f32(duration);
            let time_listened = context.listened.lock().await.total_heard();
            time_listened >= FOUR_MINUTES ||
            time_listened.as_secs_f32() >= (length.as_secs_f32() / 2.)
        } else { false }
    }

    #[tracing::instrument(skip(self, context), level = "debug")]
    async fn set_now_listening(&mut self, context: super::BackendContext<crate::data_fetching::AdditionalTrackData>) {
        // TODO: catch network errors and add to a queue.
        if let Some(track_data) = Self::basic_track_metadata(&context.track) {
            let additional_info = Self::additional_info(&context.track, &context.app, self.client.get_program_info());
            let started_listening_at = if let Some(at) = context.listened.lock().await.started_at() { at } else { tracing::error!("no start duration for current listening"); return };
            if let Err(error) = self.client.submit_listen(track_data, started_listening_at, Some(additional_info)).await {
                tracing::error!(?error, "listenbrainz now-listening failure")
            }
        } else {
            tracing::warn!("listenbrainz now-listening dispatch skipped; track is missing required data (artist name)")
        }
    }
}

