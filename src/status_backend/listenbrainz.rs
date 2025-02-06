use std::sync::{Arc, LazyLock};

// use listenbrainz::raw::{request::{ListenType, Payload, SubmitListens, TrackMetadata}, Client as ListenBrainzClient};
use apple_music::Track;
use maybe_owned_string::{MaybeOwnedStringDeserializeToOwned, MaybeOwnedString};
use serde::{Serialize, Deserialize};

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

    fn basic_track_metadata(track: &Track) -> brainz::listen::v1::submit_listens::BasicTrackMetadata<'_> {
        brainz::listen::v1::submit_listens::BasicTrackMetadata {
            artist: &track.artist,
            track: &track.name,
            release: Some(&track.album)
        }
    }

    fn additional_info<'a>(track: &'a Track, app: &'a apple_music::ApplicationData, program: &'a brainz::music::request_client::ProgramInfo<S>) -> brainz::listen::v1::submit_listens::additional_info::AdditionalInfo<'a> {
        brainz::listen::v1::submit_listens::additional_info::AdditionalInfo {
            duration: Some(core::time::Duration::from_millis((track.duration * 1000.) as u64)),
            track_number: Some(track.track_number as u32),
            submission_client: Some(program),
            music_service: Some(brainz::listen::v1::submit_listens::additional_info::MusicService::Domain("music.apple.com")),
            media_player: Some(brainz::listen::v1::submit_listens::additional_info::MediaPlayer {
                name: "Apple Music",
                version: app.version.as_deref(),
            }),
            ..Default::default()
        }
    }
}
#[async_trait::async_trait]
impl StatusBackend for ListenBrainz {
    #[tracing::instrument(level = "debug")]   
    async fn record_as_listened(&self, track: Arc<Track>, app: Arc<apple_music::ApplicationData>) {
        // TODO: catch net error or add to queue. ideally queue persist offline
        self.client.submit_playing_now(
            Self::basic_track_metadata(&track),
            Some(Self::additional_info(&track, &app, self.client.get_program_info()))
        ).await;
    }

    /// - <https://listenbrainz.readthedocs.io/en/latest/users/api/core.html#post--1-submit-listens>
    async fn check_eligibility(&self, track: Arc<Track>, time_listened: &core::time::Duration) -> bool {
        let length = core::time::Duration::from_secs_f64(track.duration);
        time_listened >= &FOUR_MINUTES ||
        time_listened.as_secs_f64() >= (length.as_secs_f64() / 2.)
    }

    #[tracing::instrument(level = "debug")]
    async fn set_now_listening(&mut self, track: Arc<Track>, app: Arc<apple_music::ApplicationData>, _: Arc<crate::data_fetching::AdditionalTrackData>) {
        self.client.submit_playing_now(
            Self::basic_track_metadata(&track),
            Some(Self::additional_info(&track, &app, self.client.get_program_info()))
        ).await;
    }
}

