#![doc = include_str!("../README.md")]
#![allow(unused)]
use std::{collections::HashMap, fmt::Debug, hash::Hash, io::{Cursor, Read, Seek, SeekFrom}, marker::PhantomData, ops::Deref, path::Path, pin::Pin, ptr::null};
use byteorder::{LittleEndian, ReadBytesExt};

pub mod id;
pub mod boma;
pub mod units;
pub use id::*;
mod version;
use boma::*;
use flate2::read;
use mzstatic::image::MzStaticImage;
use maybe_owned_string::MaybeOwnedString;
use serde::Deserialize;
use version::AppleMusicVersion;
use unaligned_u16::utf16::Utf16Str;

const ENCRYPTION_KEY: &[u8] = b"BHUILuilfghuila3";
#[cfg(not(feature = "tracing"))]
mod tracing {
    // mock
    pub struct Span;
    impl Span {
        pub fn in_scope<T>(self, f: impl FnOnce() -> T) -> T {
            f()
        }
    }

    macro_rules! debug_span {
        ($name: expr) => {
            tracing::Span
        };
    }
    macro_rules! _warn {
        ($($arg: tt)*) => {};
    }
    
    pub(crate) use debug_span;
    pub(crate) use _warn as warn;
}

pub(crate) fn convert_timestamp(seconds: u32) -> Option<chrono::DateTime<chrono::Utc>> {
    if seconds == 0 { return None }
    
    use chrono::TimeZone;

    const EPOCH_OFFSET: i64 = 2082819600;

    Some(chrono::Utc.timestamp_opt(seconds as i64 - EPOCH_OFFSET, 0).unwrap())
}

pub(crate) struct Reader<'a> {
    pub buffer: &'a[u8],
    pub cursor: Cursor<&'a[u8]>,
    pub version: Option<AppleMusicVersion>
}
impl<'a> Reader<'a> {
    pub fn peek<'b>(&mut self, mut buffer: &'b mut [u8]) -> Result<&'b mut [u8], std::io::Error> {
        let len = Read::read(&mut self.cursor, buffer)?;
        self.backtrack(len as i64)?;
        Ok(&mut buffer[..len])
    }
    pub fn backtrack(&mut self, amount: i64) -> Result<u64, std::io::Error> {
        self.advance(-amount)
    }
    pub fn advance(&mut self, amount: i64) -> Result<u64, std::io::Error> {
        self.cursor.seek(SeekFrom::Current(amount))
    }
    pub fn read_signature(&mut self) -> [u8; 4] {
        let mut signature: [u8; 4] = [0_u8; 4];
        Read::read(&mut self.cursor, &mut signature).expect("can't read signature");
        signature
    }
    pub fn read_sequence<'b, T: ContextlessRead<'a>>(&'b mut self, amount: usize) -> SequenceReader<'b, 'a, T> {
        SequenceReader::new(self, amount)
    }
    pub fn read_slice(&mut self, amount: usize) -> Result<&'a [u8], std::io::Error> {
        let position = self.cursor.position() as usize;
        let slice = &self.buffer[position..position + amount];
        self.advance(slice.len() as i64)?;
        Ok(slice)
    }

    pub fn get_ptr(&self) -> *const u8 {
        unsafe {
            self.buffer.as_ptr().add(self.cursor.position() as usize)
        }
    }

    pub fn new_versionless(buffer: &'a[u8]) -> Self {
        let cursor = Cursor::new(buffer);
        Self { buffer, cursor, version: None }
    }

    pub fn new(buffer: &'a[u8], version: AppleMusicVersion) -> Self {
        let cursor = Cursor::new(buffer);
        Self { buffer, cursor, version: Some(version) }
    }
}    

pub(crate) struct SequenceReader<'a, 'b, T: ContextlessRead<'b>> {
    reader: &'a mut Reader<'b>,
    remaining: usize,
    _type: PhantomData<T>
}
impl<'a, 'b, T: ContextlessRead<'b>> SequenceReader<'a, 'b, T> {
    fn new(reader: &'a mut Reader<'b>, remaining: usize) -> Self {
        Self { reader, remaining, _type: PhantomData }
    }
}
impl<'b, T: ContextlessRead<'b>> Iterator for SequenceReader<'_, 'b, T> {
    type Item = Result<T, T::ReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let read = T::read(self.reader);
        self.remaining -= 1;
        Some(read)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.remaining))
    }
}


#[derive(Debug)]
struct Header {
    file_size: u32,
    max_crypt_size: u32,
    header_size: u32,


    apple_music_version: AppleMusicVersion,

    track_count: u32,
    playlist_count: u32,
    collection_count: u32,
    artist_count: u32,
}
impl Header {
    pub fn get_encrypted_data_size(&self) -> usize {
        if self.max_crypt_size < self.file_size {
            self.max_crypt_size as usize
        } else {
            let data_size = self.file_size - self.header_size;
            (data_size - (data_size % 16)) as usize
        }
    }
}
impl<'a> ContextlessRead<'a> for Header {
    const SIGNATURE: &'static [u8; 4] = b"hfma";
    type ReadError = std::io::Error;
    
    fn read_contents(reader: &mut Reader<'a>) -> Result<Self, Self::ReadError> where Self: Sized {

        let mut v = [0; 165];
        reader.peek(&mut v);

        let header_size: u32 = reader.cursor.read_u32::<LittleEndian>()?;
        let file_size: u32 = reader.cursor.read_u32::<LittleEndian>()?;

        reader.advance(4);

        let apple_music_version = {
            let ptr = reader.get_ptr();
            let mut buffer = [0; 32];
            reader.cursor.read_exact(&mut buffer)?;
            let null_terminator = buffer.iter().enumerate().find(|(_, v)| **v == 0).expect("version did not terminate").0;
            let str = unsafe {
                let slice = core::slice::from_raw_parts(ptr, null_terminator);
                core::str::from_utf8_unchecked(slice)
            };
            str.parse().expect("bad version")
        };

        reader.cursor.set_position(60);
        // dbg!(reader.cursor.read_u32::<LittleEndian>()?);
        // dbg!(reader.cursor.read_u32::<LittleEndian>()?);


        // reader.cursor.set_position(68);
        let track_count = reader.cursor.read_u32::<LittleEndian>()?;
        let playlist_count = reader.cursor.read_u32::<LittleEndian>()?;
        let collection_count = reader.cursor.read_u32::<LittleEndian>()?;
        let artist_count = reader.cursor.read_u32::<LittleEndian>()?;


        reader.cursor.set_position(84);
        let max_crypt_size = reader.cursor.read_u32::<LittleEndian>()?;
        Ok(Header {
            file_size,
            header_size,
            max_crypt_size,

            // db_version,
            apple_music_version,

            track_count,
            playlist_count,
            collection_count,
            artist_count
        })
    }
}

#[derive(thiserror::Error, Debug)]
pub enum PreliminaryReadError {
    #[error("header read failure: {0}")]
    Header(std::io::Error),
    #[error("decryption failure: {0}")]
    Decryption(aes::cipher::block_padding::UnpadError),
    #[error("decompression failure: {0}")]
    Decompression(std::io::Error)
}

pub(crate) fn decode(data: &mut [u8]) -> Result<(Header, Vec<u8>), PreliminaryReadError> {
    let mut reader = Reader::new_versionless(data);
    let header = Header::read(&mut reader).map_err(PreliminaryReadError::Header)?;
    let data = unsafe {
        // Because of the presence of a borrowed string within the header, we're not able to simply
        // obtain mutable access to the data. However, we do know that it is safe as we'll only ever
        // be mutating any data *after* the position of the header.
        //
        // (That's the point of the [`core::slice::split_at`] method and related, except we can't use that here because
        //  we treat the header as not having a defined length until we read the length from within it.)
        let ptr = data as *const _ as *mut [u8];
        let data = &mut *ptr; // disassociate from previous borrow
        &mut data[header.header_size as usize..]
    };
    
    let (encrypted, unencrypted) = data.split_at_mut(header.get_encrypted_data_size());

    let decrypted = tracing::debug_span!("decryption").in_scope(|| {
        use ecb::cipher::{KeyInit, BlockDecryptMut};
        type Padding = aes::cipher::block_padding::NoPadding;
        type Decryptor = ecb::Decryptor<aes::Aes128>;
        Decryptor::new(ENCRYPTION_KEY.into())
            .decrypt_padded_mut::<Padding>(encrypted)
            .map_err(PreliminaryReadError::Decryption)
    })?;

    struct ReadableDualJoined<'a> {
        second: &'a [u8],
        current: &'a [u8],
        index: usize,
    }
    impl<'a> ReadableDualJoined<'a> {
        fn new(a: &'a [u8], b: &'a [u8]) -> Self {
            Self { current: a, second: b, index: 0 }
        }
    }
    impl Read for ReadableDualJoined<'_> {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            if self.index == self.current.len() {
                if self.current == self.second {
                    return Ok(0);
                } else {
                    self.current = self.second;
                    self.index = 0;
                }
            }
            let size = buffer.len();
            let read = size.min(self.current.len() - self.index);
            buffer[..read].copy_from_slice(&self.current[self.index..][..read]);
            self.index += read;
            Ok(read)
        }
    }

    let compressed = ReadableDualJoined::new(decrypted, unencrypted);
    let compressed_size = decrypted.len() + unencrypted.len();
    let decompressed = tracing::debug_span!("decompression").in_scope(|| {
        use flate2::read::ZlibDecoder;
        const TYPICAL_COMPRESSION_FACTOR: usize = 10; // TODO: Figure out what a good value to use is.
        let mut uncompressed = Vec::with_capacity(compressed_size * TYPICAL_COMPRESSION_FACTOR);
        ZlibDecoder::new(compressed)
            .read_to_end(&mut uncompressed)
            .map_err(PreliminaryReadError::Decompression)
            .map(|_| uncompressed)
    })?;

    Ok((header, decompressed))
}

pub(crate) trait ContextlessRead<'a> {
    type ReadError: std::error::Error;
    const SIGNATURE: &'static [u8; 4];

    fn is_signature_ahead(reader: &mut Reader<'a>) -> Result<bool, std::io::Error> {
        Ok(reader.peek(&mut [0; 4])? == Self::SIGNATURE)
    }

    fn read_if_present(reader: &mut Reader<'a>) -> Result<Option<Self>, Self::ReadError> where Self: Sized {
        if Self::is_signature_ahead(reader).unwrap_or_default() {
            Self::read(reader).map(Some)
        } else {
            Ok(None)
        }
    }

    fn read_contents(reader: &mut Reader<'a>) -> Result<Self, Self::ReadError> where Self: Sized;
    fn read(reader: &mut Reader<'a>) -> Result<Self, Self::ReadError> where Self: Sized {
        let signature = reader.read_signature();
        assert_eq!(&signature, Self::SIGNATURE, "invalid header @0x{:X} ({}), expected {} got {}",
            reader.cursor.position() - 4,
            reader.cursor.position() - 4,
            String::from_utf8_lossy(Self::SIGNATURE),
            String::from_utf8_lossy(&signature)
        );
        Self::read_contents(reader)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ListReadError<T: core::fmt::Debug> {
    #[error("bad item: {0}")]
    BadItem(T),
    #[error("bad list header: {0}")]
    BadListHeader(std::io::Error),
}

pub struct List<'a, T>(Vec<T>, PhantomData<&'a ()>);
#[allow(private_bounds)]
impl<'a, T: ContextlessRead<'a>> List<'a, T> {
    pub(crate) fn read_contents(reader: &mut Reader<'a>) -> Result<Self, ListReadError<<T as ContextlessRead<'a>>::ReadError>> {
        let byte_length = reader.cursor.read_u32::<LittleEndian>().map_err(ListReadError::BadListHeader)?;
        let item_count = reader.cursor.read_u32::<LittleEndian>().map_err(ListReadError::BadListHeader)? as usize;
        reader.advance(byte_length as i64 - 12).map_err(ListReadError::BadListHeader)?;
        let mut items = Vec::with_capacity(item_count);
        for item in reader.read_sequence::<T>(item_count) {
            items.push(item.map_err(ListReadError::BadItem)?);
        }
        Ok(Self(items, PhantomData))
    }
}
impl<'a, T: ContextlessRead<'a>> core::fmt::Debug for List<'a, T> where T: Debug {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("List").field(&self.0).finish()
    }
}
impl<'a, T: ContextlessRead<'a>> Deref for List<'a, T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<'a, T: ContextlessRead<'a>> IntoIterator for List<'a, T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

pub struct Map<'a, T: id::persistent::Possessor>(HashMap<T::Id, T>, PhantomData<&'a ()>);
impl<'a, T: id::persistent::Possessor> Map<'a, T> {
    pub(crate) fn read_contents(reader: &mut Reader<'a>) -> Result<Self, ListReadError<<T as ContextlessRead<'a>>::ReadError>> where T: ContextlessRead<'a> {
        let byte_length = reader.cursor.read_u32::<LittleEndian>().map_err(ListReadError::BadListHeader)?;
        let item_count = reader.cursor.read_u32::<LittleEndian>().map_err(ListReadError::BadListHeader)? as usize;
        reader.advance(byte_length as i64 - 12).map_err(ListReadError::BadListHeader)?;
        let mut items = HashMap::<T::Id, T>::with_capacity(item_count);
        for item in reader.read_sequence::<T>(item_count) {
            let item = item.map_err(ListReadError::BadItem)?;
            items.insert(item.get_persistent_id(), item);
        };;
        Ok(Self(items, PhantomData))
    }
}
impl<'a, T: ContextlessRead<'a> + id::persistent::Possessor> core::fmt::Debug for Map<'a, T> where T: Debug, T::Id: Debug  {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Map ")?;
        f.debug_map().entries(self.iter()).finish()
    }
}
impl<'a, T: ContextlessRead<'a> + id::persistent::Possessor> Deref for Map<'a, T> {
    type Target = HashMap<T::Id, T>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<'a, T: ContextlessRead<'a> + id::persistent::Possessor> IntoIterator for Map<'a, T> {
    type Item = (T::Id, T);
    type IntoIter = std::collections::hash_map::IntoIter<T::Id, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
impl<'a, T: ContextlessRead<'a> + id::persistent::Possessor> IntoIterator for &Map<'a, T> where Self: 'a {
    type Item = (&'a T::Id, &'a T);
    type IntoIter = std::collections::hash_map::Iter<'a, T::Id, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}


macro_rules! derive_list {
    ($identifier: ident, $content: ty, $signature: literal) => {
        type $identifier<'a> = List<'a, $content>;

        impl<'a> ContextlessRead<'a> for $identifier<'a> {
            const SIGNATURE: &'static [u8; 4] = $signature;
            type ReadError = ListReadError<<$content as ContextlessRead<'a>>::ReadError>;
            fn read_contents(reader: &mut Reader<'a>) -> Result<Self, Self::ReadError> where Self: Sized {
                List::<'a, $content>::read_contents(reader)
            }
        }
    }
}

macro_rules! derive_map {
    ($identifier: ident, $content: ty, $signature: literal) => {
        type $identifier<'a> = Map<'a, $content>;

        impl<'a> ContextlessRead<'a> for $identifier<'a> {
            const SIGNATURE: &'static [u8; 4] = $signature;
            type ReadError = ListReadError<<$content as ContextlessRead<'a>>::ReadError>;
            fn read_contents(reader: &mut Reader<'a>) -> Result<Self, Self::ReadError> where Self: Sized {
                Map::<'a, $content>::read_contents(reader)
            }
        }
    }
}

#[repr(u32)]
#[derive(strum_macros::FromRepr, Debug)]
enum SectionBoundarySubtype {
    PlaylistMasterOrFileEntry = 3, // hsma, lPma
    LibraryMaster = 6, // plma
    AlbumList = 4, // lama
    ArtistList = 5, // lAma
    AccountData = 15, // Lsma
    TrackList = 1, // Ltma
    CollectionList = 2, // lPma
}

#[derive(thiserror::Error, Debug)]
enum SectionBoundaryError {
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("unknown subtype `{0}`")]
    UnknownSubtype(u32)
}

#[derive(Debug)]
struct SectionBoundary {
    // r0x0..3 ; b"hsma"
    next_section_offset: u32, // r0x4..7
    associated_sections_length: u32, // r0x8..12
    subtype: SectionBoundarySubtype, // r0x12..15
    // ; ...zeros, len-12
}
impl ContextlessRead<'_> for SectionBoundary {
    type ReadError = SectionBoundaryError;
    const SIGNATURE: &'static [u8; 4] = b"hsma";

    fn read_contents(reader: &mut Reader) -> Result<Self, Self::ReadError> {
        let next_section_offset = reader.cursor.read_u32::<LittleEndian>()?;
        let associated_sections_length = reader.cursor.read_u32::<LittleEndian>()?;
        let subtype = reader.cursor.read_u32::<LittleEndian>()?;
        let subtype = SectionBoundarySubtype::from_repr(subtype).ok_or(SectionBoundaryError::UnknownSubtype(subtype))?;
        reader.advance((next_section_offset - 16) as i64)?; // 12 read + 4 sig
        Ok(Self { next_section_offset, associated_sections_length, subtype })
    }
}


#[derive(Debug)]
struct HeaderRepeat {
    // r0x0..3 ; b"hfma"
    // r0x4..7 ; len
}
impl ContextlessRead<'_> for HeaderRepeat {
    type ReadError = std::io::Error;
    const SIGNATURE: &'static [u8; 4] = b"hfma";

    fn read_contents(reader: &mut Reader) -> Result<Self, Self::ReadError> {
        let length = reader.cursor.read_u32::<LittleEndian>()?;
        reader.advance(length as i64 - 8)?;
        Ok(Self {})
    }
}


derive_list!(LibraryMaster, Boma<'a>, b"plma");

#[allow(unused)]
#[derive(Debug)]
pub struct Artist<'a> {
    // r0x0..3 ; b"iAma"
    // r0x4..7 ; len
    // r0x8..11 ; associated section length
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
impl<'a> ContextlessRead<'a> for Artist<'a> {
    type ReadError = std::io::Error;
    const SIGNATURE: &'static [u8; 4] = b"iAma";

    fn read_contents(reader: &mut Reader<'a>) -> Result<Self, Self::ReadError> {
        let length = reader.cursor.read_u32::<LittleEndian>()?;
        reader.advance(4)?; // assoc length;
        let boma_count = reader.cursor.read_u32::<LittleEndian>()?;
        let persistent_id = reader.cursor.read_u64::<LittleEndian>()?.into();
        reader.advance(28)?;
        let cloud_catalog_id = reader.cursor.read_u32::<LittleEndian>()?;
        let cloud_catalog_id = core::num::NonZeroU32::new(cloud_catalog_id);
        let cloud_catalog_id = cloud_catalog_id.map(|c| unsafe { id::cloud::Catalog::new_unchecked(c) });
        reader.advance(length as i64 - 56)?;
        let mut cloud_library_id = None;
        let mut name = None;
        let mut name_sorted = None;
        let mut artwork_url = None;

        for boma in reader.read_sequence(boma_count as usize) {
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
                            .inspect_err(|err| { dbg!(err, value); })
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
    const IDENTITY: cloud::library::PossessorIdentity = cloud::library::PossessorIdentity::Artist;
}
impl id::cloud::catalog::Possessor for Artist<'_> {
    #[allow(private_interfaces)]
    const IDENTITY: cloud::catalog::PossessorIdentity = cloud::catalog::PossessorIdentity::Artist;
}
derive_map!(ArtistMap, Artist<'a>, b"lAma");

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
impl<'a> ContextlessRead<'a> for Album<'a> {
    type ReadError = std::io::Error;
    const SIGNATURE: &'static [u8; 4] = b"iama";

    fn read_contents(reader: &mut Reader<'a>) -> Result<Self, Self::ReadError> {
        let length = reader.cursor.read_u32::<LittleEndian>()?;
        reader.advance(4)?; // assoc length;
        let boma_count = reader.cursor.read_u32::<LittleEndian>()?;
        let persistent_id = reader.cursor.read_u64::<LittleEndian>()?.into();
        reader.advance(length as i64 - 24)?;
        let mut album_name = None;
        let mut artist_name = None;
        let mut artist_name_cloud = None;
        let mut cloud_library_id = None;
        for boma in reader.read_sequence::<Boma>(boma_count as usize) {
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
    const IDENTITY: cloud::catalog::PossessorIdentity = cloud::catalog::PossessorIdentity::Album;
}
impl id::cloud::library::Possessor for Album<'_> {
    #[allow(private_interfaces)]
    const IDENTITY: cloud::library::PossessorIdentity = cloud::library::PossessorIdentity::Album;
}

derive_map!(AlbumMap, Album<'a>, b"lama");

#[derive(thiserror::Error, Debug)]
pub enum TrackReadError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("missing required boma: {0:?}")]
    LackingBoma(BomaSubtype),
    #[error("invalid utf-16 string: {0}")]
    InvalidUtf16(unaligned_u16::utf16::error::InvalidUtf16)
    // #[cfg_attr(feature = "serde", error("plist deserialization error: {0}"))]
    // #[cfg(feature = "serde")] Deserialization(#[from] plist::Error),
}

// TODO: find play count >:-[
#[derive(Debug)]
#[allow(unused)]
pub struct Track<'a> {
    // bomas: Vec<Boma<'a>>,
    pub name: Option<&'a Utf16Str>,
    pub persistent_id: <Track<'a> as id::persistent::Possessor>::Id,
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
impl<'a> ContextlessRead<'a> for Track<'a> {
    const SIGNATURE: &'static [u8; 4] = b"itma";
    type ReadError = std::io::Error;

    fn read_contents(reader: &mut Reader<'a>) -> Result<Self, Self::ReadError> where Self: Sized {
        
        
        let length = reader.cursor.read_u32::<LittleEndian>()?;

        // // let mut jor = vec![0; length as usize];
        // // reader.cursor.read_exact(&mut jor[..])?;
        // // println!("itma {:?}", &jor[..]);
        // // reader.cursor.seek(SeekFrom::Current(-(length as i64)))?;


        reader.advance(4)?; // ?
        let boma_count = reader.cursor.read_u32::<LittleEndian>()?;
        let persistent_id = reader.cursor.read_u64::<LittleEndian>()?.into();
        reader.advance(148)?; // ?
        // hey why aren't the below Optional ???? is it a bunch of zeros if not existing?
        let album_id = reader.cursor.read_u64::<LittleEndian>()?.into();
        let artist_id = reader.cursor.read_u64::<LittleEndian>()?.into();
        reader.advance((length as i64) - 188)?;


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

        macro_rules! match_boma_utf16_or {
            ($boma: expr, [$(($variant: ident, $variable: ident)$(,)?)*], $fallback: expr) => {
                match $boma {
                    $(Boma::Utf16(BomaUtf16(value, BomaUtf16Variant::$variant)) => { $variable = Some(value) }),*
                    boma => $fallback(boma)
                }
            }
        }

        for boma in reader.read_sequence(boma_count as usize) {
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
                    Boma::Utf8Xml(BomaUtf8(v, BomaUtf8Variant::PlistCloudDownloadInformation)) => {
                        // cloud universal library id, redownload params 
                    } 
                    Boma::Utf8Xml(BomaUtf8(_, BomaUtf8Variant::TrackLocalFilePathUrl)) => {},
                    boma => {
                        let subtype = boma.get_subtype();
                        // IDK what 23 is yet
                        if subtype != Err(UnknownBomaError(23)) {
                            tracing::warn!("unexpected unknown boma {:?} on {persistent_id:?}", boma.get_subtype());
                        }
                    }
                }
            });
        }


        Ok(Self {
            artwork,
            name,
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
            numerics: numerics.unwrap(),
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
    const IDENTITY: cloud::catalog::PossessorIdentity = cloud::catalog::PossessorIdentity::Track;
}
impl id::cloud::library::Possessor for Track<'_> {
    #[allow(private_interfaces)]
    const IDENTITY: cloud::library::PossessorIdentity = cloud::library::PossessorIdentity::Track;
}

impl<'a> Track<'a> {
    pub fn get_artist_on(&'a self, artists: impl Into<&'a ArtistMap<'a>> + 'a) -> Option<&'a Artist<'a>> {
        Into::<&'a ArtistMap<'a>>::into(artists).get(&self.artist_id)
    }
    pub fn get_album_on(&'a self, albums: impl Into<&'a AlbumMap<'a>> + 'a) -> Option<&'a Album<'a>> {
        Into::<&'a AlbumMap<'a>>::into(albums).get(&self.album_id)
    }
}


derive_map!(TrackMap, Track<'a>, b"ltma");
#[derive(Debug)]
pub struct Account<'a> {
    bomas: Vec<Boma<'a>>,
    pub persistent_id: <Self as id::persistent::Possessor>::Id,
}
impl<'a> ContextlessRead<'a> for Account<'a> {
    const SIGNATURE: &'static [u8; 4] = b"isma";
    type ReadError = std::io::Error;

    fn read_contents(reader: &mut Reader<'a>) -> Result<Self, Self::ReadError> where Self: Sized {
        let length = reader.cursor.read_u32::<LittleEndian>()?;
        reader.advance(4)?; // ?
        let boma_count = reader.cursor.read_u32::<LittleEndian>()?;
        let persistent_id = reader.cursor.read_u64::<LittleEndian>()?.into();
        reader.advance((length as i64) - 24)?;
        let bomas = reader.read_sequence::<Boma>(boma_count as usize).collect::<Result<_, _>>()?;
        Ok(Self { bomas, persistent_id })
    }
}
impl<'a> id::persistent::Possessor for Account<'a> {
    #[allow(private_interfaces)]
    const IDENTITY: id::persistent::PossessorIdentity = id::persistent::PossessorIdentity::Account;
    type Id = PersistentId<Account<'a>>;
    fn get_persistent_id(&self) -> Self::Id {
        self.persistent_id
    }
}
derive_list!(AccountInfoList, Account<'a>, b"Lsma");




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


enum CollectionType {
    Library, // contains all songs
    Apple,
    User,
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
impl<'a> ContextlessRead<'a> for Collection<'a> {
    const SIGNATURE: &'static [u8; 4] = b"lpma";
    type ReadError = CollectionReadError<'a>;

    fn read_contents(reader: &mut Reader<'a>) -> Result<Self, Self::ReadError> where Self: Sized {
        let slice = &mut [0; 300];
        let slice = reader.peek(slice)?;


        let length = reader.cursor.read_u32::<LittleEndian>()?;
        reader.advance(4)?;
        let boma_count = reader.cursor.read_u32::<LittleEndian>()?;
        let track_count = reader.cursor.read_u32::<LittleEndian>()?;
        reader.advance(26 - (12 + 4))?;
        let persistent_id = reader.cursor.read_u64::<LittleEndian>()?.into();
        reader.advance(40 - (26 + 8))?;
        let is_master = reader.cursor.read_u8()? == 1;
        reader.advance(134 - (40 + 1))?;
        let modification_date = convert_timestamp(reader.cursor.read_u32::<LittleEndian>()?);
        reader.advance(186 - (134 + 4))?;
        let v = reader.cursor.read_u16::<LittleEndian>()? == 257;



        reader.advance(300 - (186 + 2))?;
        let creation_date = convert_timestamp(reader.cursor.read_u32::<LittleEndian>()?);


        reader.advance((length as i64) - (304 + 4))?;
        let mut tracks = Vec::with_capacity(track_count as usize);
        let mut name = None;
        let mut info = None::<CollectionInfo<'a>>;



        for boma in reader.read_sequence(boma_count as usize) {
            match boma? {
                Boma::Utf16(BomaUtf16(new_name, BomaUtf16Variant::PlaylistName)) => name = Some(new_name),
                Boma::Utf8Xml(BomaUtf8(read_info, BomaUtf8Variant::PlistPlaylistInfo)) => info = Some(CollectionInfo::try_from(read_info).map_err(CollectionReadError::Deserialization)?),
                Boma::CollectionMember(member) => tracks.push(member),
                boma => {
                    // 201 has magic "SLst" header
                    // tracing::warn!("Unexpected subtype present: {:?}", boma.get_subtype());
                }
            }
        }
        let name = name.ok_or(CollectionReadError::LackingBoma(BomaUtf16Variant::PlaylistName.into()))?;

        Ok(Self { name, info, tracks, persistent_id, creation_date, modification_date })
    }
}
impl<'a> Collection<'a> {
    pub fn get_tracks_on<'b: 'a>(&self, tracks: &'a TrackMap<'a>) -> Vec<Option<&'a Track>> {
        self.tracks.iter()
            .map(|member| tracks.get(&member.track_persistent_id))
            .collect::<Vec<_>>()
    }
}
impl<'a> id::persistent::Possessor for Collection<'a> {
    type Id = PersistentId<Collection<'a>>;
    #[allow(private_interfaces)]
    const IDENTITY: id::persistent::PossessorIdentity = id::persistent::PossessorIdentity::Collection;
    fn get_persistent_id(&self) -> Self::Id {
        self.persistent_id
    }
}

derive_list!(CollectionMap, Collection<'a>, b"lPma");

#[derive(Debug)]
pub struct CollectionMember<'a> {
    pub track_persistent_id: <Track<'a> as id::persistent::Possessor>::Id
}
impl CollectionMember<'_> {
    pub const BOMA_SUBTYPE: u32 = 206;

    pub(crate) fn read_content(reader: &mut Reader) -> Result<Self, std::io::Error> {
        reader.advance(4)?;
        assert_eq!(&reader.read_signature(), b"ipfa");
        let length = reader.cursor.read_u32::<LittleEndian>()?;
        reader.advance(12)?;
        let track_persistent_id = reader.cursor.read_u64::<LittleEndian>()?.into();
        reader.cursor.seek(SeekFrom::Current((length as i64) - 28))?;
        Ok(Self { track_persistent_id })
    }
}

trait DbAccess<'a> {
    fn get<T: id::persistent::Possessor>(&self, id: PersistentId<T>) -> Option<&'a T>;

    fn library(&self) -> &LibraryMaster<'a>;
    fn albums(&self) -> &AlbumMap<'a>;
    fn artists(&self) -> &ArtistMap<'a>;
    fn accounts(&self) -> Option<&AccountInfoList<'a>>;
    fn tracks(&self) -> &TrackMap<'a>;
    fn collections(&self) -> &CollectionMap<'a>;
}

#[derive(Debug)]
pub struct MusicDbView<'a> {
    pub library: LibraryMaster<'a>,
    pub albums: AlbumMap<'a>,
    pub artists: ArtistMap<'a>,
    /// All of the Apple Music accounts associated with the storage.
    // Wasn't present on a Windows copy, but that might be because they've only logged in as one user.
    // For some god-forsaken reason beyond any comprehension, my personal laptop has had *two* associated
    // accounts, one of whom is a rapper and DJ from the UK? So, uh, needs more research.
    pub accounts: Option<AccountInfoList<'a>>,
    pub tracks: TrackMap<'a>,
    pub collections: CollectionMap<'a>
}
impl<'a> MusicDbView<'a> {
    pub(crate) fn with_reader(mut reader: Reader<'a>) -> Self {
        macro_rules! expect_boundary {
            ($reader: ident) => {
                SectionBoundary::read(&mut $reader).expect("can't read section boundary");        
            }
        }
        
        expect_boundary!(reader);
        HeaderRepeat::read(&mut reader).expect("can't read header duplicate");

        expect_boundary!(reader);
        let library = LibraryMaster::read(&mut reader).expect("can't read library master");

        expect_boundary!(reader);
        let albums = AlbumMap::read(&mut reader).expect("can't read albums list");

        expect_boundary!(reader);
        let artists = ArtistMap::read(&mut reader).expect("can't read artists list");

        expect_boundary!(reader);
        let accounts = AccountInfoList::read_if_present(&mut reader).expect("can't read account list");

        if accounts.is_some() { expect_boundary!(reader); }
        let tracks = TrackMap::read(&mut reader).expect("can't read track list");

        expect_boundary!(reader);
        let collections = CollectionMap::read(&mut reader).expect("can't read collection list");

        Self {
            library,
            albums,
            artists,
            accounts,
            tracks,
            collections
        }
    }

    /// Returns the value with the given ID (be it a track, album, artist, et cetera).
    /// 
    /// Only works for IDs with their datatype attached at the type-level, such as IDs which were retrieved from the DB itself.
    #[allow(clippy::missing_transmute_annotations)]
    fn get<T: id::persistent::Possessor>(&self, id: PersistentId<T>) -> Option<&'a T> {
        match T::IDENTITY {
            id::persistent::PossessorIdentity::Account => {
                let id: PersistentId<Account<'a>> = unsafe { core::mem::transmute(id) };
                if self.accounts.is_none() {
                    tracing::warn!("account ID passed without existence of accounts field");
                };
                let account = self.accounts.as_ref().and_then(|accounts| {
                    accounts.iter().find(|account| account.persistent_id == id)
                 });
                unsafe { core::mem::transmute(account) }
            }
            id::persistent::PossessorIdentity::Album => {
                let id: PersistentId<Album<'a>> = unsafe { core::mem::transmute(id) };
                let album = self.albums.get(&id);
                unsafe { core::mem::transmute(album) }
            },
            id::persistent::PossessorIdentity::Artist => {
                let id: PersistentId<Artist<'a>> = unsafe { core::mem::transmute(id) };
                let artist = self.artists.get(&id);
                unsafe { core::mem::transmute(artist) }
            },
            id::persistent::PossessorIdentity::Collection => {
                let id: PersistentId<Collection<'a>> = unsafe { core::mem::transmute(id) };
                let collection = &self.collections.0.iter().find(|collection| collection.persistent_id == id);
                unsafe { core::mem::transmute(collection) }
            },
            id::persistent::PossessorIdentity::Track => {
                let id: PersistentId<Track<'a>> = unsafe { core::mem::transmute(id) };
                let track = self.tracks.get(&id);
                unsafe { core::mem::transmute(track) }
            },
        }
    }
}
impl<'a> From<&'a MusicDbView<'a>> for &'a AlbumMap<'a> {
    fn from(value: &'a MusicDbView<'a>) -> Self {
        &value.albums
    }
}
impl<'a> From<&'a MusicDbView<'a>> for &'a ArtistMap<'a> {
    fn from(value: &'a MusicDbView<'a>) -> Self {
        &value.artists
    }
}
impl<'a> From<&'a MusicDbView<'a>> for &'a TrackMap<'a> {
    fn from(value: &'a MusicDbView<'a>) -> Self {
        &value.tracks
    }
}
impl<'a> From<&'a MusicDbView<'a>> for &'a CollectionMap<'a> {
    fn from(value: &'a MusicDbView<'a>) -> Self {
        &value.collections
    }
}

pub struct MusicDB {
    _owned_data: Pin<Vec<u8>>,
    view: MusicDbView<'static>, // not really static; lifetime is 'self (as long as `_owned_data` exists)
    path: std::path::PathBuf
}
impl MusicDB {
    pub fn read_path(path: impl AsRef<Path>) -> MusicDB {
        let path = path.as_ref().to_path_buf();
        let data = &mut std::fs::read(&path).unwrap()[..];
        let (header, data) = decode(data).unwrap();
        let data = Pin::new(data);

        // Obtain a slice of the data with a lifetime promoted to that of the returned instance (not actually 'static, but 'self).
        // SAFETY:
        //  - The data is behind [`core::pin::Pin`], meaning the memory address won't ever change.
        //  - The data will be owned by the returned struct, so if it's dropped, the view would already be invalidated as the lifetime would've expired.
        //  - The data is contiguous, and can safely be mapped to a slice.
        let slice: &'static [u8] = unsafe {
            let addr = data.as_ptr();
            core::slice::from_raw_parts::<'static, u8>(addr, data.len())
        };

        let view  = Reader::new(slice, header.apple_music_version);
        let view = MusicDbView::with_reader(view);

        Self { view, _owned_data: data, path }
    }
    pub fn extract_raw(path: impl AsRef<Path>) -> Result<Vec<u8>, std::io::Error> {
        let data = &mut std::fs::read(&path)?;
        let (_, data) = decode(data).unwrap();
        Ok(data)
    }
    pub fn get_view(&self) -> &MusicDbView<'_> {
        // 'static => 'self
        unsafe { core::mem::transmute(&self.view) }
    }
    pub fn update_view(&mut self)  {
        // TODO: Persistent handle? I dunno.
        *self = Self::read_path(self.path.as_path())
    }
    pub fn default_path() -> std::path::PathBuf {
        #[allow(deprecated)] // This binary is MacOS-exclusive; this function only has unexpected behavior on Windows.
        let home = std::env::home_dir().unwrap();
        home.as_path().join("Music/Music/Music Library.musiclibrary/Library.musicdb")
    }
}
impl core::default::Default for MusicDB {
    fn default() -> Self {
        MusicDB::read_path(MusicDB::default_path())
    }
}
impl core::fmt::Debug for MusicDB {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MusicDB")
            .field("path", &self.path)
            .field("view", &self.view)
            .finish()
    }
}
impl MusicDB {
    /// Returns the value with the given ID (be it a track, album, artist, et cetera).
    /// 
    /// Only works for IDs with their datatype attached at the type-level, such as IDs which were retrieved from the DB itself.
    pub fn get<T: id::persistent::Possessor>(&self, id: PersistentId<T>) -> Option<&T> {
        self.get_view().get(id)
    }

    /// Returns a map of every album in the library.
    pub fn albums(&self) -> &AlbumMap<'_> {
        &self.get_view().albums
    }
    /// Returns a map of every artist in the library.
    pub fn artists(&self) -> &ArtistMap<'_> {
        &self.get_view().artists
    }
    /// Returns a map of every track in the library.
    pub fn tracks(&self) -> &TrackMap<'_> {
        &self.get_view().tracks
    }
    /// Returns a map of every collection (playlist) in the library.
    pub fn collections(&self) -> &CollectionMap<'_> {
        &self.get_view().collections
    }
    /// Returns a map of every account associated with the library.
    /// This isn't always present.
    pub fn accounts(&self) -> Option<&AccountInfoList<'_>> {
        self.get_view().accounts.as_ref()
    }
}

pub(crate) fn xxd(mut slice: &[u8]) -> String {
    let mut out = String::new();
    const HEX_PER_LINE: usize = 32;
    let mut n = 0;
    while !slice.is_empty() {
        let (line, rest) = slice.split_at(slice.len().min(HEX_PER_LINE));
        slice = rest;

        const ZERO_PADDING: usize = 4;
        let row = n * HEX_PER_LINE; n += 1;        
        let digits = ((row as f64).log10().ceil() as usize).max(1);
        if digits != 0 {
            out.push_str("\x1b[2;30m"); // light gray
            for digit in 0..ZERO_PADDING - digits {
                out.push('0');
            }
            out.push_str("\x1b[0m"); // reset
        }

        out.push_str(&format!("{} | ", row));


        for byte in line {
            if *byte == 0 {
                out.push_str("\x1b[2;30m00 \x1b[0m")
            } else {
                out.push_str(&format!("{:02x} ", byte));
            }
        }

        out.push('\n');
    }
    out
}
