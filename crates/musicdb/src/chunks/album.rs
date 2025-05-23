use super::derive_map;
use crate::{*, chunk::*};

#[derive(Debug)]
pub struct Album<'a> {
    // r0x0..3 ; b"iama"
    // r0x4..7 ; len
    // r0x8..11 ; associated section length
    // r0x12..15 ; boma count
    pub persistent_id: <Self as id::persistent::Possessor>::Id, // r0x16..23
    pub album_name: Option<&'a Utf16Str>,
    pub artist_name: Option<&'a Utf16Str>,
    pub artist_name_cloud: Option<&'a Utf16Str>,
    pub cloud_library_id: Option<id::cloud::Library<Album<'a>, &'a Utf16Str>>
}
impl<'a> Chunk for Album<'a> {
    const SIGNATURE: Signature = Signature::new(*b"iama");
}
impl<'a> SizedFirstReadableChunk<'a> for Album<'a> {
    type ReadError = std::io::Error;

    fn read_sized_content(cursor: &mut std::io::Cursor<&'a [u8]>, offset: u64, length: u32) -> Result<Self, Self::ReadError> {
        setup_eaters!(cursor, offset, length);
        skip!(4)?; // appendage byte length
        let boma_count = u32!()?;
        let persistent_id = id!(Album)?;
        skip_to_end!()?;

        let mut album_name = None;
        let mut artist_name = None;
        let mut artist_name_cloud = None;
        let mut cloud_library_id = None;
        
        for boma in cursor.reading_chunks::<Boma>(boma_count as usize) {
            match boma? {
                Boma::Utf16(BomaUtf16(value, BomaUtf16Variant::IamaAlbum)) => album_name = Some(value),
                Boma::Utf16(BomaUtf16(value, BomaUtf16Variant::IamaAlbumArtist)) => artist_name = Some(value),
                Boma::Utf16(BomaUtf16(value, BomaUtf16Variant::IamaAlbumArtistCloud)) => artist_name_cloud = Some(value),
                Boma::Utf16(BomaUtf16(value, BomaUtf16Variant::IamaAlbumCloudId)) => {
                    cloud_library_id = Some(unsafe { id::cloud::Library::new_unchecked(value) });
                },
                _ => panic!("unknown") // fixme good error handling
            }
        }
        Ok(Self {
            album_name,
            artist_name,
            artist_name_cloud,
            persistent_id,
            cloud_library_id,
        })
    }
}
impl<'a> id::persistent::Possessor for Album<'a> {
    type Id = PersistentId<Album<'a>>;
    #[allow(private_interfaces)]
    const IDENTITY: id::persistent::PossessorIdentity = id::persistent::PossessorIdentity::Album;
    fn get_persistent_id(&self) -> Self::Id {
        self.persistent_id
    }
}
impl id::cloud::catalog::Possessor for Album<'_> {
    #[allow(private_interfaces)]
    const IDENTITY: id::cloud::catalog::PossessorIdentity = id::cloud::catalog::PossessorIdentity::Album;
}
impl id::cloud::library::Possessor for Album<'_> {
    #[allow(private_interfaces)]
    const IDENTITY: id::cloud::library::PossessorIdentity = id::cloud::library::PossessorIdentity::Album;
}

derive_map!(pub AlbumMap, Album<'a>, *b"lama");
