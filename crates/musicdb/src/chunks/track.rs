use maybe_owned_string::MaybeOwnedString;
use mzstatic::image::MzStaticImage;

use crate::{boma::*, chunk::*, id, setup_eaters, PersistentId, Utf16Str};
use super::{album::Album, artist::Artist, derive_map, AlbumMap, ArtistMap};


#[derive(thiserror::Error, Debug)]
pub enum TrackReadError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("missing required boma: {0:?}")]
    LackingBoma(BomaSubtype),
    // #[error("invalid utf-16 string: {0}")]
    // InvalidUtf16(unaligned_u16::utf16::error::InvalidUtf16)
    // #[cfg_attr(feature = "serde", error("plist deserialization error: {0}"))]
    // #[cfg(feature = "serde")] Deserialization(#[from] plist::Error),
}

// TODO: find play count >:-[
#[derive(Debug)]
#[allow(unused)]
pub struct Track<'a> {
    pub name: Option<&'a Utf16Str>,
    pub persistent_id: <Track<'a> as id::persistent::Possessor>::Id,
    pub cloud_id: Option<id::cloud::Library<Track<'a>, MaybeOwnedString<'a>>>,
    pub album_id: <Album<'a> as id::persistent::Possessor>::Id,
    pub album_name: Option<&'a Utf16Str>,
    pub album_artist_name: Option<&'a Utf16Str>,
    pub artist_id: <Artist<'a> as id::persistent::Possessor>::Id,
    pub artist_name: Option<&'a Utf16Str>,
    pub genre: Option<&'a Utf16Str>,
    pub sort_order_name: Option<&'a Utf16Str>,
    pub sort_order_album_name: Option<&'a Utf16Str>,
    pub sort_order_album_artist_name: Option<&'a Utf16Str>,
    pub sort_order_artist_name: Option<&'a Utf16Str>,
    pub sort_order_composer: Option<&'a Utf16Str>,

    pub artwork: Option<MzStaticImage<'a>>,


    pub numerics: TrackNumerics<'a>,
    pub composer: Option<&'a Utf16Str>,
    pub kind: Option<&'a Utf16Str>,
    pub copyright: Option<&'a Utf16Str>,
    pub comment: Option<&'a Utf16Str>,

    // also appears on downloading for offline
    pub purchaser_email: Option<&'a Utf16Str>,
    pub purchaser_name: Option<&'a Utf16Str>,
    pub grouping: Option<&'a Utf16Str>,
    pub classical_work_name: Option<&'a Utf16Str>,
    pub classical_movement_title: Option<&'a Utf16Str>,
    pub fairplay_info: Option<&'a Utf16Str>,
    // appears on downloading for offline, maybe purchasing? no examples to test
    pub local_file_path: Option<&'a Utf16Str>,
}
impl Chunk for Track<'_> {
    const SIGNATURE: Signature = Signature::new(*b"itma");
}
impl<'a> SizedFirstReadableChunk<'a> for Track<'a> {
    type ReadError = TrackReadError;

    fn read_sized_content(cursor: &mut std::io::Cursor<&'a [u8]>, offset: u64, length: u32) -> Result<Self, Self::ReadError> {
        setup_eaters!(cursor, offset, length);
        skip!(4)?; // appendage byte length
        let boma_count = u32!()?;
        let persistent_id = id!(Track)?;
        skip!(148)?;
        // hey why aren't the below Optional ???? is it a bunch of zeros if not existing?
        let album_id = id!(Album)?;
        let artist_id = id!(Artist)?;
        skip_to_end!()?;


        let mut album_name = None;
        let mut name = None;
        let mut artist_name = None;
        let mut genre = None;
        let mut album_artist_name = None;
        let mut sort_order_name = None;
        let mut sort_order_album_name = None;
        let mut sort_order_album_artist_name = None;
        let mut sort_order_artist_name = None;
        let mut sort_order_composer = None;
        let mut numerics = None;
        let mut composer = None;
        let mut kind = None;
        let mut copyright = None;
        let mut comment = None;
        let mut purchaser_email = None;
        let mut purchaser_name = None;
        let mut grouping = None;
        let mut classical_work_name = None;
        let mut classical_movement_title = None;
        let mut fairplay_info = None;
        let mut artwork = None;
        let mut local_file_path = None;
        let mut cloud_id = None;

        macro_rules! match_boma_utf16_or {
            ($boma: expr, [$(($variant: ident, $variable: ident)$(,)?)*], $fallback: expr) => {
                match $boma {
                    $(Boma::Utf16(BomaUtf16(value, BomaUtf16Variant::$variant)) => { $variable = Some(value) }),*
                    boma => $fallback(boma)
                }
            }
        }

        for boma in cursor.reading_chunks::<Boma>(boma_count as usize) {
            match_boma_utf16_or!(boma?, [
                (Album, album_name),
                (AlbumArtist, album_artist_name),
                (Artist, artist_name),
                (Composer, composer),
                (CopyrightHolder, copyright),
                (TrackTitle, name),
                (Kind, kind),
                (Genre, genre),
                (SortOrderTrackTitle, sort_order_name),
                (SortOrderArtist, sort_order_artist_name),
                (SortOrderAlbum, sort_order_album_name),
                (SortOrderAlbumArtist, sort_order_album_artist_name),
                (SortOrderComposer, sort_order_composer),
                (Comment, comment),
                (PurchaserEmail, purchaser_email),
                (PurchaserName, purchaser_name),
                (Grouping, grouping),
                (ClassicalMovementTitle, classical_movement_title),
                (ClassicalWorkName, classical_work_name),
                (FairPlayInfo, fairplay_info),
                (TrackLocalFilePath, local_file_path)
            ], |boma| {
                match boma {
                    Boma::TrackNumerics(value) => numerics = Some(value),
                    Boma::Book(_) => (),
                    Boma::Utf8Xml(BomaUtf8(value, BomaUtf8Variant::PlistTrackCloudInformation)) => {
                        use serde::Deserialize as _;

                        #[derive(serde::Deserialize, Debug)]
                        #[serde(rename_all = "kebab-case", bound = "'a: 'de, 'de: 'a")] //
                        #[allow(unused)]
                        struct Raw<'a> {
                            cloud_album_id: Option<MaybeOwnedString<'a>>,
                            cloud_artwork_token: Option<MaybeOwnedString<'a>>,
                            cloud_artist_id: Option<MaybeOwnedString<'a>>,
                            cloud_artwork_url: Option<MaybeOwnedString<'a>>,
                            cloud_lyrics: Option<MaybeOwnedString<'a>>,
                            cloud_lyrics_tokens: Option<MaybeOwnedString<'a>>
                        }


                        let mut deserializer = plist::serde::Deserializer::parse(value).unwrap().expect("a value should be present");
                        let raw = Raw::deserialize(&mut deserializer).unwrap(); // TODO: Handle
                    
                        artwork = raw.cloud_artwork_token.and_then(|v| MzStaticImage::with_pool_and_token(v).ok())
                    }
                    Boma::Utf8Xml(BomaUtf8(value, BomaUtf8Variant::PlistCloudDownloadInformation)) => {
                        use serde::Deserialize as _;

                        #[derive(serde::Deserialize, Debug)]
                        #[serde(rename_all = "kebab-case", bound = "'a: 'de, 'de: 'a")] //
                        #[allow(unused)]
                        struct Raw<'a> {
                            redownload_params: Option<MaybeOwnedString<'a>>,
                            cloud_universal_library_id: Option<MaybeOwnedString<'a>>,
                        }


                        let mut deserializer = plist::serde::Deserializer::parse(value).unwrap().expect("a value should be present");
                        let raw = Raw::deserialize(&mut deserializer).unwrap(); // TODO: Handle
                        cloud_id = raw.cloud_universal_library_id.and_then(|v| unsafe { id::cloud::Library::new_unchecked(v) }.into());
                    } 
                    Boma::Utf8Xml(BomaUtf8(_, BomaUtf8Variant::TrackLocalFilePathUrl)) => {},
                    boma => {
                        let subtype = boma.get_subtype();
                        // IDK what 23 is yet
                        if subtype != Err(UnknownBomaError(23)) {
                            #[cfg(feature = "tracing")]
                            tracing::warn!("unexpected unknown boma {:?} on {persistent_id:?}", boma.get_subtype());
                        }
                    }
                }
            });
        }


        Ok(Self {
            artwork,
            name,
            cloud_id,
            album_id,
            album_name,
            persistent_id,
            artist_name,
            artist_id,
            album_artist_name,
            genre,
            sort_order_name,
            sort_order_album_name,
            sort_order_album_artist_name,
            sort_order_artist_name,
            sort_order_composer,
            numerics: numerics.ok_or(TrackReadError::LackingBoma(BomaSubtype::TrackNumerics))?,
            composer,
            kind,
            copyright,
            comment,
            purchaser_email,
            purchaser_name,
            grouping,
            classical_movement_title,
            classical_work_name,
            fairplay_info,
            local_file_path
        })
    }
}
impl<'a> id::persistent::Possessor for Track<'a> {
    type Id = PersistentId<Track<'a>>;
    #[allow(private_interfaces)]
    const IDENTITY: id::persistent::PossessorIdentity = id::persistent::PossessorIdentity::Track;
    fn get_persistent_id(&self) -> Self::Id {
        self.persistent_id
    }
}
impl id::cloud::catalog::Possessor for Track<'_> {
    #[allow(private_interfaces)]
    const IDENTITY: id::cloud::catalog::PossessorIdentity = id::cloud::catalog::PossessorIdentity::Track;
}
impl id::cloud::library::Possessor for Track<'_> {
    #[allow(private_interfaces)]
    const IDENTITY: id::cloud::library::PossessorIdentity = id::cloud::library::PossessorIdentity::Track;
}

impl<'a> Track<'a> {
    pub fn get_artist_on(&'a self, artists: impl Into<&'a ArtistMap<'a>> + 'a) -> Option<&'a Artist<'a>> {
        Into::<&'a ArtistMap<'a>>::into(artists).get(&self.artist_id)
    }
    pub fn get_album_on(&'a self, albums: impl Into<&'a AlbumMap<'a>> + 'a) -> Option<&'a Album<'a>> {
        Into::<&'a AlbumMap<'a>>::into(albums).get(&self.album_id)
    }
}

derive_map!(pub TrackMap, Track<'a>, *b"ltma");

