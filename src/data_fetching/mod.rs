pub mod services;
pub mod components;

use components::{Component, ComponentSolicitation};
use components::artwork::TrackArtworkData;

#[derive(Debug)]
#[allow(dead_code, reason = "used only by certain featured-gated backends")]
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
        let itunes = if solicitation.contains(Component::ITunesData) {
            services::itunes::find_track(&services::itunes::Query {
                title: track.name.as_ref(),
                artist: track.artist.as_deref(),
                album: track.album.as_deref()
            }).await.inspect_err(|error| tracing::error!(?error, %track.persistent_id, "failed to get iTunes data")).ok().flatten()
        } else { None };

        Self {
            images: artwork_manager.get(&solicitation, track, itunes.as_ref(),
                #[cfg(feature = "musicdb")]
                musicdb
            ).await,
            itunes,
        }
    }
}

