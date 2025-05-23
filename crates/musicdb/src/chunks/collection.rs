use maybe_owned_string::MaybeOwnedString;

use crate::{boma::*, chunk::*, convert_timestamp, id, setup_eaters, PersistentId, Utf16Str};
use super::{derive_list, track::Track};


#[derive(thiserror::Error, Debug)]
pub enum CollectionReadError<'a> {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("missing required boma: {0:?}")]
    LackingBoma(BomaSubtype),
    #[error("plist deserialization error: {0}")]
    Deserialization(plist::serde::Error<'a>),
}

#[derive(Debug)]
pub struct CollectionInfo<'a> {
    pub owner: Option<(Option<u32>, MaybeOwnedString<'a>)>, // no ID for (own?) user playlists
    pub description: Option<MaybeOwnedString<'a>>,
}
impl<'a> TryFrom<&'a str> for CollectionInfo<'a> {
    type Error = plist::serde::Error<'a>;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        use serde::Deserialize as _;

        // there is literally not one single property that is there 100% of the time
        // jesus christ this shit is a mess
        #[derive(serde::Deserialize, Debug)]
        #[serde(rename_all = "kebab-case", bound = "'a: 'de, 'de: 'a")]
        #[allow(unused)]
        struct Raw<'a> {
            external_container_tag: Option<MaybeOwnedString<'a>>,
            external_vendor_display_name: Option<MaybeOwnedString<'a>>,
            generated_artwork_uuids: Option<Vec<MaybeOwnedString<'a>>>,
            cloud_artwork_token: Option<MaybeOwnedString<'a>>,
            cloud_artwork_url: Option<MaybeOwnedString<'a>>,
            cover_artwork_recipe: Option<MaybeOwnedString<'a>>,
            description: Option<MaybeOwnedString<'a>>,
            #[serde(rename = "ownerID")]
            owner_id: Option<MaybeOwnedString<'a>>,
            #[serde(rename = "ownerName")]
            owner_name: Option<MaybeOwnedString<'a>>,
            subscribed_container_url: Option<MaybeOwnedString<'a>>,
            universal_library_id: Option<MaybeOwnedString<'a>>,
            version_hash: Option<MaybeOwnedString<'a>>, // 256 bit (32 hex)
            /// sometimes /pl\.[0-9a-f]{32}/ (uuid no dashes)
            /// sometimes /pl\.u-\w{15}/  ( what)
            /// sometimes literally fucking nonsense
            /// last part of `subscribed-container-url`'s path (if present? idk if uuid always implies that exists; todo: check)
            uuid: Option<MaybeOwnedString<'a>>,
        }


        let mut deserializer = plist::serde::Deserializer::parse(value)?.expect("a value should be present");
        let raw = Raw::deserialize(&mut deserializer)?;

        Ok(CollectionInfo {
            description: raw.description,
            owner: raw.owner_name.map(|name| (
                raw.owner_id.map(|v| v.as_ref().parse().unwrap()),
                name,
            )),
        })
    }
}

#[derive(Debug)]
pub struct Collection<'a> {
    pub name: &'a Utf16Str,
    pub info: Option<CollectionInfo<'a>>, // not present on collection w/ name "Hidden Cloud PlaylistOnly Tracks"
    pub tracks: Vec<CollectionMember<'a>>,
    pub persistent_id: <Self as id::persistent::Possessor>::Id,
    pub creation_date: Option<chrono::DateTime<chrono::Utc>>,
    pub modification_date: Option<chrono::DateTime<chrono::Utc>>,
}
impl<'a> Chunk for Collection<'a> {
    const SIGNATURE: Signature = Signature::new(*b"lpma");
}

impl<'a> SizedFirstReadableChunk<'a> for Collection<'a> {
    type ReadError = CollectionReadError<'a>;

    fn read_sized_content(cursor: &mut std::io::Cursor<&'a [u8]>, offset: u64, length: u32) -> Result<Self, Self::ReadError> {
        setup_eaters!(cursor, offset, length);
        skip!(4)?; // appendage byte length
        let boma_count = u32!()?;
        let track_count = u32!()?;
        skip!(26 - (12 + 4))?;
        let persistent_id = id!(Collection)?;
        skip!(40 - (26 + 8))?;
        let _is_master = u8!()? == 1;
        skip!(134 - (40 + 1))?;
        let modification_date = convert_timestamp(u32!()?);
        skip!(186 - (134 + 4))?;
        // let v = reader.cursor.read_u16::<LittleEndian>()? == 257;
        skip!(300 - (186 + 2))?;
        let creation_date = convert_timestamp(u32!()?);


        skip_to_end!()?;
        let mut tracks = Vec::with_capacity(track_count as usize);
        let mut name = None;
        let mut info = None::<CollectionInfo<'a>>;

        for boma in cursor.reading_chunks::<Boma>(boma_count as usize) {
            match boma? {
                Boma::Utf16(BomaUtf16(new_name, BomaUtf16Variant::PlaylistName)) => name = Some(new_name),
                Boma::Utf8Xml(BomaUtf8(read_info, BomaUtf8Variant::PlistPlaylistInfo)) => info = Some(CollectionInfo::try_from(read_info).map_err(CollectionReadError::Deserialization)?),
                Boma::CollectionMember(member) => tracks.push(member),
                _boma => {
                    // 201 has magic "SLst" header
                    #[cfg(feature = "tracing")]
                    tracing::warn!("Unexpected subtype present: {:?}", _boma.get_subtype());
                }
            }
        }
        let name = name.ok_or(CollectionReadError::LackingBoma(BomaUtf16Variant::PlaylistName.into()))?;

        Ok(Self { name, info, tracks, persistent_id, creation_date, modification_date })
    }
}
// impl<'a> Collection<'a> {
//     pub fn get_tracks_on<'b: 'a>(&self, tracks: &'a TrackMap<'a>) -> Vec<Option<&'a Track>> {
//         self.tracks.iter()
//             .map(|member| tracks.get(&member.track_persistent_id))
//             .collect::<Vec<_>>()
//     }
// }
impl<'a> id::persistent::Possessor for Collection<'a> {
    type Id = PersistentId<Collection<'a>>;
    #[allow(private_interfaces)]
    const IDENTITY: id::persistent::PossessorIdentity = id::persistent::PossessorIdentity::Collection;
    fn get_persistent_id(&self) -> Self::Id {
        self.persistent_id
    }
}
impl id::cloud::catalog::Possessor for Collection<'_> {
    #[allow(private_interfaces)]
    const IDENTITY: id::cloud::catalog::PossessorIdentity = id::cloud::catalog::PossessorIdentity::Collection;
}
impl id::cloud::library::Possessor for Collection<'_> {
    #[allow(private_interfaces)]
    const IDENTITY: id::cloud::library::PossessorIdentity = id::cloud::library::PossessorIdentity::Collection;
}

#[derive(Debug)]
pub struct CollectionMember<'a> {
    pub track_persistent_id: <Track<'a> as id::persistent::Possessor>::Id
}
impl CollectionMember<'_> {
    pub const BOMA_SUBTYPE: u32 = 206;

    pub(crate) fn read_content(cursor: &mut std::io::Cursor<&[u8]>) -> Result<Self, std::io::Error> {
        use byteorder::ReadBytesExt as _;
        use byteorder::LittleEndian as LE;
        use std::io::Seek as _;
        cursor.advance(4)?;
        assert_eq!(&cursor.read_signature()?, b"ipfa");
        let length = cursor.read_u32::<LE>()?;
        cursor.advance(12)?;
        let track_persistent_id = cursor.read_u64::<LE>()?.into();
        cursor.seek(std::io::SeekFrom::Current((length as i64) - 28))?;
        Ok(Self { track_persistent_id })
    }
}

derive_list!(pub CollectionMap, Collection<'a>, *b"lPma");
