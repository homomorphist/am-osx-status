use musicdb::MusicDB;

use super::artworkd::StoredArtwork; // todo: not import from there // <-- wtf did i mean when i typed that

#[derive(thiserror::Error, Debug)]
pub enum MusicDbTrackArtworkRetrievalFailure {
    #[error("cannot convert to persistent id: {0}")]
    IdConversionFailure(#[from] core::num::ParseIntError),
    #[error("no track with the given persistent id could be found")]
    NoTrackWithId,
}

// Ok(None) = track exists but no artwork
pub fn get_track_artwork(musicdb: &MusicDB, track: &osa_apple_music::track::Track) -> Result<Option<StoredArtwork>, MusicDbTrackArtworkRetrievalFailure> {
    let db = musicdb.get_view();
    let id = AsRef::<str>::as_ref(&track.persistent_id).try_into()?;
    let track = db.tracks.get(&id).ok_or(MusicDbTrackArtworkRetrievalFailure::NoTrackWithId)?;
    Ok(track.artwork.as_ref().map(|artwork| StoredArtwork::Remote { url: artwork.to_string() }))
}
