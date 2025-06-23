pub mod services;

use components::{image::TrackImageUrlPack, Component, ComponentSolicitation};
use services::{artworkd::{get_artwork, StoredArtwork}};

pub mod components;

#[cfg(feature = "musicdb")]
type MusicDB = musicdb::MusicDB;
#[cfg(not(feature = "musicdb"))]
type MusicDB = ();


#[derive(Debug)]
pub struct AdditionalTrackData {
    pub itunes: Option<itunes_api::Track>,
    pub images: TrackImageUrlPack
}
impl AdditionalTrackData {
    pub async fn from_solicitation(
        solicitation: ComponentSolicitation,
        track: &crate::status_backend::DispatchableTrack,
        musicdb: Option<&MusicDB>,
        host: Option<&mut Box<dyn crate::data_fetching::services::custom_artwork_host::CustomArtworkHost>>,
    ) -> Self {
        let mut itunes: Option<itunes_api::Track> = None;
        let mut images = TrackImageUrlPack::none();
        let id = track.persistent_id.as_str();

        if solicitation.list.contains(&Component::ITunesData) {
            itunes = match services::itunes::find_track(track).await {
                Ok(itunes) => itunes,
                Err(err) => {
                    tracing::error!(?err, %id, "failed to get iTunes data");
                    None
                }
            }
        }

        #[cfg(feature = "musicdb")]
        if solicitation.list.contains(&Component::ArtistImage) && let Some(db) = musicdb {
            let id = musicdb::PersistentId::try_from(id).expect("bad id");
            images.artist = db.tracks().get(&id)
                .and_then(|track| db.get(track.artist_id))
                .and_then(|artist| artist.artwork_url.as_ref())
                .filter(|artwork| artwork.parameters.effect != Some(mzstatic::image::effect::Effect::SquareFitCircle)) // ugly auto-generated
                .map(|artwork| artwork.to_string())
        }

        if solicitation.list.contains(&Component::AlbumImage) {
             if let Some(itunes) = itunes.as_ref() {
                images.track = itunes.artwork_mzstatic().map(|mut mzstatic|{
                    use mzstatic::image::quality::Quality;
                    mzstatic.parameters.quality = Some(Quality::new(500).unwrap());
                    mzstatic.to_string()
                }).ok()
            }
            
            #[cfg(feature = "musicdb")]
            if images.track.is_none() && let Some(db) = musicdb {
                let id = musicdb::PersistentId::try_from(id).expect("bad id");
                images.track = db.tracks().get(&id)
                    .and_then(|track| track.artwork.as_ref())
                    .map(|artwork| artwork.to_string())
            }

            if images.track.is_none() {
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
        }


        Self {
            itunes,
            images
        }
    }
}

