pub mod services;

use components::{image::TrackImageUrlPack, Component, ComponentSolicitation};
use services::{artworkd::{get_artwork, StoredArtwork}, itunes::ITunesStoreSong};

use crate::util::fallback_to_default_and_log_error;

pub mod components;

#[derive(Debug)]
pub struct AdditionalTrackData {
    pub itunes: Option<ITunesStoreSong>,
    pub images: TrackImageUrlPack
}
impl AdditionalTrackData {
    pub async fn from_solicitation(
        solicitation: ComponentSolicitation,
        track: &crate::status_backend::DispatchableTrack,
        musicdb: Option<&musicdb::MusicDB>,
        host: Option<&mut Box<dyn crate::data_fetching::services::custom_artwork_host::CustomArtworkHost>>,
    ) -> Self {
        let mut itunes: Option<ITunesStoreSong> = None;
        let mut images = TrackImageUrlPack::none();

        if solicitation.list.contains(&Component::ArtistImage) {
            if let Some(db) = musicdb {
                let id = musicdb::PersistentId::try_from(track.persistent_id.as_str()).expect("bad id");
                images.artist = db.tracks().get(&id)
                    .and_then(|track| db.get(track.artist_id))
                    .and_then(|artist| artist.artwork_url.as_ref())
                    .and_then(|artwork| {
                        if artwork.parameters.effect == Some(mzstatic::image::effect::Effect::SquareFitCircle) {
                            None // ugly auto-generated
                        } else {
                            Some(artwork.to_string())
                        }
                    });
            }
        }

        if solicitation.list.contains(&Component::AlbumImage) {
            images.track = match crate::util::fallback_to_default_and_log_error!(get_artwork(&track.persistent_id)) {
                None => None,
                Some(artwork) => match artwork {
                    StoredArtwork::Remote { url } => Some(url),
                    StoredArtwork::Local { path } => {
                        if let Some(host) = host  {
                            host.for_track(track, &path).await.inspect_err(|err| {
                                tracing::error!(?err, "failed to upload custom artwork");
                            }).ok()
                        } else {
                            None
                        }
                    },
                }
            };
        }

        if solicitation.list.contains(&Component::ITunesData) {
            itunes = fallback_to_default_and_log_error!(services::itunes::find_track(track).await);
        }

        Self {
            itunes,
            images
        }
    }

}
