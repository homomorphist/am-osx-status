pub mod services;

use components::{Component, ComponentSolicitation};
use components::artwork::TrackArtworkData;
use services::artworkd::get_artwork;

pub mod components;

#[derive(Debug)]
pub struct AdditionalTrackData {
    pub itunes: Option<itunes_api::Track>,
    pub images: TrackArtworkData
}
impl AdditionalTrackData {
    pub async fn from_solicitation(
        solicitation: ComponentSolicitation,
        track: &crate::subscribers::DispatchableTrack,
        #[cfg(feature = "musicdb")]
        musicdb: Option<&musicdb::MusicDB>,
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
            images: artwork_manager.get(&solicitation, track, itunes.as_ref(),
                #[cfg(feature = "musicdb")]
                musicdb
            ).await,
            itunes,
        }
    }
}

