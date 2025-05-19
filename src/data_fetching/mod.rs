pub mod services;

use components::{image::TrackImageUrlPack, Component, ComponentSolicitation};
use services::{artworkd::{get_artwork, StoredArtwork}, itunes::ITunesStoreSong};

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
        let id = track.persistent_id.as_str();

        if solicitation.list.contains(&Component::ArtistImage) {
            if let Some(db) = musicdb {
                let id = musicdb::PersistentId::try_from(id).expect("bad id");
                images.artist = db.tracks().get(&id)
                    .and_then(|track| db.get(track.artist_id))
                    .and_then(|artist| artist.artwork_url.as_ref())
                    .filter(|artwork| artwork.parameters.effect != Some(mzstatic::image::effect::Effect::SquareFitCircle)) // ugly auto-generated
                    .map(|artwork| artwork.to_string())
            }
        }

        if solicitation.list.contains(&Component::AlbumImage) {
            let artwork = match get_artwork(id).await {
                Ok(artwork) => artwork,
                Err(err) => {
                    tracing::error!(?err, %id, "failed to get artwork");
                    None
                }
            };

            images.track = match artwork {
                None => None,
                Some(artwork) => match artwork {
                    StoredArtwork::Remote { url } => Some(url),
                    StoredArtwork::Local { path } => {
                        if let Some(host) = host  {
                            host.for_track(track, &path).await.inspect_err(|err| {
                                tracing::error!(?err, %id, "failed to upload custom artwork");
                            }).ok()
                        } else {
                            None
                        }
                    },
                }
            };
        }

        if solicitation.list.contains(&Component::ITunesData) {
            itunes = match services::itunes::find_track(track).await {
                Ok(itunes) => itunes,
                Err(err) => {
                    tracing::error!(?err, %id, "failed to get iTunes data");
                    None
                }
            }
        }

        Self {
            itunes,
            images
        }
    }

}
