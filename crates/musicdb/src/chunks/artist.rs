use crate::{boma::*, chunk::*, id, setup_eaters, PersistentId, Utf16Str};

use super::derive_map;

#[allow(unused)]
#[derive(Debug)]
pub struct Artist<'a> {
    // r0x0..3 ; b"iAma"

    // r0x12..15 ; boma count
    pub persistent_id: <Artist::<'a> as id::persistent::Possessor>::Id, // r0x16..23
    /// e.x. 1147783278; see <https://developer.apple.com/documentation/applemusicapi/get-a-catalog-artist#Example>
    pub cloud_catalog_id: Option<id::cloud::Catalog<Artist<'a>>>,
    /// e.x. "r.y8mMT7t"; see <https://developer.apple.com/documentation/applemusicapi/get-a-library-artist#Example>
    pub cloud_library_id: Option<id::cloud::Library<Artist<'a>, &'a Utf16Str>>,

    pub name: Option<&'a Utf16Str>,
    pub name_sorted: Option<&'a Utf16Str>,
    pub artwork_url: Option<mzstatic::image::MzStaticImage<'a>>
}
impl Chunk for Artist<'_> {
    const SIGNATURE: Signature = Signature::new(*b"iAma");
}
impl<'a> SizedFirstReadableChunk<'a> for Artist<'a> {
    type ReadError = std::io::Error;

    fn read_sized_content(cursor: &mut std::io::Cursor<&'a [u8]>, offset: u64, length: u32) -> Result<Self, Self::ReadError> {
        setup_eaters!(cursor, offset, length);
        skip!(4)?; // appendage byte length
        let boma_count = u32!()?;
        let persistent_id = id!(Artist)?;
        skip!(28)?;
        let cloud_catalog_id = u32!()?;
        let cloud_catalog_id: Option<std::num::NonZero<u32>> = core::num::NonZeroU32::new(cloud_catalog_id);
        let cloud_catalog_id = cloud_catalog_id.map(|c| unsafe { id::cloud::Catalog::new_unchecked(c) });
        skip_to_end!()?;

        
        let mut cloud_library_id = None;
        let mut name = None;
        let mut name_sorted = None;
        let mut artwork_url = None;
        
        for boma in cursor.reading_chunks::<Boma>(boma_count as usize) {
            match boma? {
                Boma::Utf16(BomaUtf16(value, BomaUtf16Variant::ArtistsArtistName)) => name = Some(value),
                Boma::Utf16(BomaUtf16(value, BomaUtf16Variant::ArtistsArtistNameSorted)) => name_sorted = Some(value),
                Boma::Utf16(BomaUtf16(value, BomaUtf16Variant::ArtistsArtistCloudLibraryId)) => {
                    cloud_library_id = Some(unsafe { id::cloud::Library::new_unchecked(value) })
                },
                Boma::Utf8Xml(BomaUtf8(mut value, BomaUtf8Variant::PlistArtworkURL)) => {
                    // very rigid and robust code
                    value = &value["<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n".len()..];
                    if value.starts_with("\t<key>artwork-url</key>\n\t<string>") {
                        value = &value["\t<key>artwork-url</key>\n\t<string>".len()..];
                        value = &value[..value.len() - "</string>\n</dict>\n</plist>\n".len()];
                        artwork_url = mzstatic::image::MzStaticImage::parse(value)
                            .inspect_err(|error| { tracing::error!(?error, %value, "bad artwork URL"); })
                            .ok();
                    }
                },
                _ => unimplemented!()
            };
        }
        
        Ok(Self {
            persistent_id,
            cloud_library_id,
            cloud_catalog_id,
            name,
            name_sorted,
            artwork_url
        })
    }
}
impl<'a> id::persistent::Possessor for Artist<'a> {
    type Id = PersistentId<Artist<'a>>;
    #[allow(private_interfaces)]
    const IDENTITY: id::persistent::PossessorIdentity = id::persistent::PossessorIdentity::Artist;
    fn get_persistent_id(&self) -> Self::Id {
        self.persistent_id
    }
}
impl id::cloud::library::Possessor for Artist<'_> {
    #[allow(private_interfaces)]
    const IDENTITY: id::cloud::library::PossessorIdentity = id::cloud::library::PossessorIdentity::Artist;
}
impl id::cloud::catalog::Possessor for Artist<'_> {
    #[allow(private_interfaces)]
    const IDENTITY: id::cloud::catalog::PossessorIdentity = id::cloud::catalog::PossessorIdentity::Artist;
}

derive_map!(pub ArtistMap, Artist<'a>, *b"lAma");
