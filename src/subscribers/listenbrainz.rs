use std::sync::Arc;
use maybe_owned_string::MaybeOwnedStringDeserializeToOwned;

use super::{error::dispatch::DispatchError, DispatchableTrack, subscribe};
use crate::{data_fetching::AdditionalTrackData, listened::TimeDeltaExtension as _};

const FOUR_MINUTES: chrono::TimeDelta = chrono::TimeDelta::new(4 * 60, 0).unwrap();

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

use brainz::listen::v1::submit_listens::ListenSubmissionError;
impl From<ListenSubmissionError> for DispatchError {
    fn from(error: ListenSubmissionError) -> Self {
        match error {
            ListenSubmissionError::NetworkFailure(err) => err.into(),
            ListenSubmissionError::HistoricDateError(_) => DispatchError::invalid_data("date of listen is too far in the past"),
            ListenSubmissionError::InvalidToken(_) => DispatchError::unauthorized(Some("invalid token")),
            ListenSubmissionError::Ratelimited => todo!("ratelimited"),
            ListenSubmissionError::Other(..) => todo!(),
        }
    }
}

use brainz::listen::v1::submit_listens::CurrentlyPlayingSubmissionError;
impl From<CurrentlyPlayingSubmissionError> for DispatchError {
    fn from(error: CurrentlyPlayingSubmissionError) -> Self {
        match error {
            CurrentlyPlayingSubmissionError::NetworkFailure(err) => err.into(),
            CurrentlyPlayingSubmissionError::InvalidToken(_) => DispatchError::unauthorized(Some("invalid token")),
            CurrentlyPlayingSubmissionError::Ratelimited => todo!("ratelimited"),
            CurrentlyPlayingSubmissionError::Other(..) => todo!(),
        }
    }
}

super::subscription::define_subscriber!(pub ListenBrainz, {
    client: Arc<brainz::listen::v1::Client<S>>,
});
impl core::fmt::Debug for ListenBrainz {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct(Self::NAME).finish()
    }
}
impl ListenBrainz {
    pub fn new(program_info: ProgramInfo<MaybeOwnedStringDeserializeToOwned<'static>>, token: brainz::listen::v1::UserToken) -> Self {
        Self { client: Arc::new(brainz::listen::v1::Client::new(program_info, Some(token))) }
    }

    fn basic_track_metadata(track: &DispatchableTrack) -> Result<brainz::listen::v1::submit_listens::BasicTrackMetadata<'_>, DispatchError> {
        Ok(brainz::listen::v1::submit_listens::BasicTrackMetadata {
            artist: track.artist.as_deref().ok_or(DispatchError::missing_required_data("artist name"))?,
            track: &track.name,
            release: track.album.as_deref()
        })
    }

    fn additional_info<'a>(track: &'a DispatchableTrack, app: &'a osa_apple_music::application::ApplicationData, program: &'a brainz::music::request_client::ProgramInfo<S>) -> brainz::listen::v1::submit_listens::additional_info::AdditionalInfo<'a> {
        use brainz::listen::v1::submit_listens::additional_info::*;
        AdditionalInfo {
            duration: track.duration,
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

    /// - <https://listenbrainz.readthedocs.io/en/latest/users/api/core.html#post--1-submit-listens>
    async fn is_eligible_for_submission<T>(&self, context: &super::BackendContext<T>) -> bool {
        if let Some(duration) = context.track.duration {
            let time_listened = context.listened.lock().await.total_heard();
            time_listened >= FOUR_MINUTES ||
            time_listened.as_secs_f32() >= (duration.as_secs_f32() / 2.)
        } else { false }
    }
}
subscribe!(ListenBrainz, TrackStarted, {
    async fn dispatch(&mut self, context: super::BackendContext<AdditionalTrackData>) -> Result<(), DispatchError> {
        let track_data = Self::basic_track_metadata(&context.track)?;
        let additional_info = Self::additional_info(&context.track, &context.app, self.client.get_program_info());
        self.client.submit_playing_now(track_data, Some(additional_info)).await.map_err(Into::into)
    }
});
subscribe!(ListenBrainz, TrackEnded, {
    async fn dispatch(&mut self, context: super::BackendContext<()>) -> Result<(), DispatchError> {
        if !self.is_eligible_for_submission(&context).await { return Ok(()) }
        let track_data = Self::basic_track_metadata(&context.track)?;
        let additional_info = Self::additional_info(&context.track, &context.app, self.client.get_program_info());
        let started_listening_at = context.listened.lock().await.started_at().ok_or(DispatchError::missing_required_data("listen start time"))?;
        self.client.submit_listen(track_data, started_listening_at, Some(additional_info)).await.map_err(Into::into)
    }
});
