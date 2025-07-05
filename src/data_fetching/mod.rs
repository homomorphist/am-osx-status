pub mod services;

use components::{Component, ComponentSolicitation};
use components::artwork::TrackArtworkData;
use services::artworkd::get_artwork;

pub mod components;

#[cfg(feature = "musicdb")]
type MusicDB = musicdb::MusicDB;
#[cfg(not(feature = "musicdb"))]
type MusicDB = ();


#[derive(Debug)]
pub struct AdditionalTrackData {
    pub itunes: Option<itunes_api::Track>,
    pub images: TrackArtworkData
}
impl AdditionalTrackData {
    pub async fn from_solicitation(
        solicitation: ComponentSolicitation,
        track: &crate::status_backend::DispatchableTrack,
        musicdb: Option<&MusicDB>,
        artwork_manager: std::sync::Arc<components::artwork::ArtworkManager>
    ) -> Self {
        let mut itunes: Option<itunes_api::Track> = None;

        if solicitation.list.contains(&Component::ITunesData) {
            itunes = match services::itunes::find_track(track).await {
                Ok(itunes) => itunes,
                Err(err) => {
                    tracing::error!(?err, %track.persistent_id, "failed to get iTunes data");
                    None
                }
            }
        }

        Self {
            images: artwork_manager.get(&solicitation, track, itunes.as_ref(), musicdb).await,
            itunes,
        }
    }
}

