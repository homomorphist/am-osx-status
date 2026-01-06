pub mod services;
pub mod components;

use components::{Component, ComponentSolicitation};
use components::artwork::TrackArtworkData;

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
        artwork_manager: alloc::sync::Arc<components::artwork::ArtworkManager>
    ) -> Self {
        let mut itunes: Option<itunes_api::Track> = None;

        if solicitation.list.contains(&Component::ITunesData) {
            itunes = match services::itunes::find_track(&services::itunes::Query {
                title: track.name.as_ref(),
                artist: track.artist.as_deref(),
                album: track.album.as_deref()
            }).await {
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

