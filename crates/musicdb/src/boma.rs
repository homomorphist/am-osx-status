#![allow(unused)]
use core::str;
use std::io::{Cursor, Read, Seek, SeekFrom};


use byteorder::{LittleEndian, ReadBytesExt};
use crate::Utf16Str;

use crate::chunk::{Chunk, ReadableChunk, SizedFirstReadableChunk};
use crate::chunks::CollectionMember;
use crate::version::AppleMusicVersion;
use crate::{cloud, setup_eaters};

use super::{convert_timestamp};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnknownBomaError(pub u32);
impl std::error::Error for UnknownBomaError {}
impl core::fmt::Display for UnknownBomaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown boma subtype '{}'", self.0)
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BomaSubtype {
    TrackNumerics,
    TrackPlayStatistics,
    CollectionItemMember,
    Book(BookVariant),
    Utf16(BomaUtf16Variant),
    Utf8Xml(BomaUtf8Variant),
}
impl BomaSubtype {
    pub fn get_raw(self) -> u32 {
        match self {
            Self::TrackNumerics => TrackNumerics::BOMA_SUBTYPE,
            Self::TrackPlayStatistics => TrackPlayStatistics::BOMA_SUBTYPE,
            Self::CollectionItemMember => CollectionMember::BOMA_SUBTYPE,
            Self::Utf16(variant) => variant as u32,
            Self::Utf8Xml(variant) => variant as u32,
            Self::Book(variant) => variant as u32,
        }
    }

    pub const fn is_recognized_unknown(self) -> bool {
        match self {
            Self::Utf16(variant) => variant.is_recognized_unknown(),
            Self::Utf8Xml(variant) => variant.is_recognized_unknown(),
            _ => false
        }
    }
}
impl TryFrom<u32> for BomaSubtype {
    type Error = UnknownBomaError;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        if value == TrackNumerics::BOMA_SUBTYPE {
            return Ok(Self::TrackNumerics)
        }

        if value == TrackPlayStatistics::BOMA_SUBTYPE {
            return Ok(Self::TrackPlayStatistics)
        }

        if value == CollectionMember::BOMA_SUBTYPE {
            return Ok(Self::CollectionItemMember)
        }

        if let Some(variant) = BomaUtf16Variant::from_repr(value) {
            return Ok(Self::Utf16(variant))
        }

        if let Some(variant) = BookVariant::from_repr(value) {
            return Ok(Self::Book(variant))
        }

        if let Some(variant) = BomaUtf8Variant::from_repr(value) {
            return Ok(Self::Utf8Xml(variant))
        }

        Err(UnknownBomaError(value))
    }
}
impl From<BomaSubtype> for u32 {
    fn from(val: BomaSubtype) -> Self {
        val.get_raw()
    }
}
impl From<BookVariant> for BomaSubtype {
    fn from(value: BookVariant) -> Self {
        Self::Book(value)
    }
}
impl From<BomaUtf16Variant> for BomaSubtype {
    fn from(value: BomaUtf16Variant) -> Self {
        Self::Utf16(value)
    }
}
impl From<BomaUtf8Variant> for BomaSubtype {
    fn from(value: BomaUtf8Variant) -> Self {
        Self::Utf8Xml(value)
    }
}


#[derive(Debug)]
pub enum Boma<'a> {
    TrackNumerics(TrackNumerics<'a>),
    TrackPlayStatistics(TrackPlayStatistics),
    CollectionMember(CollectionMember<'a>),
    Utf16(BomaUtf16<'a>),
    Utf8Xml(BomaUtf8<'a>),
    Book(BomaBook<'a>),
    Unknown(UnknownBoma<'a>)
}
impl Chunk for Boma<'_> {
    const SIGNATURE: crate::chunk::Signature = crate::chunk::Signature::new(*b"boma");
}
impl<'a> ReadableChunk<'a> for Boma<'a> {
    type ReadError = std::io::Error;

    fn skip(cursor: &mut super::chunk::ChunkCursor<'a>) -> Result<bool, std::io::Error> {
        let signature = cursor.peek_signature()?;
        if signature != Self::SIGNATURE { return Ok(false); }
        cursor.advance(4); // signature / unknown
        cursor.advance(4); // padding
        let length = cursor.read_u32::<LittleEndian>()?;
        cursor.advance(length as i64)?;
        Ok(true)
    }

    fn read(cursor: &mut super::chunk::ChunkCursor<'a>) -> Result<Self, Self::ReadError> where Self: Sized {
        <Self as ReadableChunk>::read_signature(cursor);
        cursor.advance(4)?; // padding / unknown
        let length = cursor.read_u32::<LittleEndian>()?;
        let subtype = cursor.read_u32::<LittleEndian>()?;
        let subtype =  BomaSubtype::try_from(subtype);
        Ok(match subtype {
            Ok(subtype) => match subtype {
                BomaSubtype::TrackNumerics => Self::TrackNumerics(TrackNumerics::read_content(cursor, length)?),
                BomaSubtype::TrackPlayStatistics => Self::TrackPlayStatistics(TrackPlayStatistics::read_content(cursor, length)?),
                BomaSubtype::CollectionItemMember => Self::CollectionMember(CollectionMember::read_content(cursor)?),
                BomaSubtype::Utf16(variant) => Self::Utf16(BomaUtf16::read_variant_content(cursor, variant).unwrap_or_else(|error| panic!("bad boma content on {variant:?}: {error:?}"))),
                BomaSubtype::Utf8Xml(variant) => Self::Utf8Xml(BomaUtf8::read_variant_content(cursor, length, variant)?),
                BomaSubtype::Book(variant) => Self::Book(BomaBook::read_variant_content(cursor, length, variant)?)
            },
            Err(UnknownBomaError(subtype)) => Self::Unknown(UnknownBoma::read_variant_content(cursor, length, subtype)?)
        })
    }
}
impl Boma<'_> {
    pub fn get_subtype(&self) -> Result<BomaSubtype, UnknownBomaError> {
        match self {
            Self::TrackNumerics(_) => Ok(BomaSubtype::TrackNumerics),
            Self::TrackPlayStatistics(_) => Ok(BomaSubtype::TrackPlayStatistics),
            Self::CollectionMember(_) => Ok(BomaSubtype::CollectionItemMember),
            Self::Utf16(BomaUtf16(_, variant)) => Ok(BomaSubtype::Utf16(*variant)),
            Self::Utf8Xml(BomaUtf8(_, variant)) => Ok(BomaSubtype::Utf8Xml(*variant)),
            Self::Book(BomaBook(_, variant)) => Ok(BomaSubtype::Book(*variant)),
            Self::Unknown(UnknownBoma { subtype, .. }) => Err(UnknownBomaError(*subtype))
        }
    }
}



#[derive(Debug)]
pub struct TrackNumerics<'a> {
    pub bitrate: Option<crate::units::KilobitsPerSecond>,
    pub date_added: Option<chrono::DateTime<chrono::Utc>>,
    pub date_modified: Option<chrono::DateTime<chrono::Utc>>,
    /// Duration of the track, in milliseconds.
    pub duration_ms: u32,

    pub cloud_catalog_album_id: Option<crate::id::cloud::Catalog<crate::chunks::Album<'a>>>,
    pub cloud_catalog_artist_id: Option<crate::id::cloud::Catalog<crate::chunks::Artist<'a>>>,
    pub cloud_catalog_track_id: Option<crate::id::cloud::Catalog<crate::chunks::Track<'a>>>,

    /// File size, in bytes.
    pub bytes: u32,
}
impl TrackNumerics<'_> {
    pub const BOMA_SUBTYPE: u32 = 0x1;

    pub fn read_content(cursor: &mut super::ChunkCursor<'_>, length: u32) -> Result<Self, std::io::Error> {
        cursor.seek(SeekFrom::Current(108 - (12 + 4)))?;
        let bitrate = cursor.read_u32::<LittleEndian>()?;
        let bitrate = if bitrate == 0 { None } else { Some(crate::units::KilobitsPerSecond(bitrate)) };
        let date_added = convert_timestamp(cursor.read_u32::<LittleEndian>()?);
        cursor.seek(SeekFrom::Current(148 - (112 + 4)))?;
        let date_modified = convert_timestamp(cursor.read_u32::<LittleEndian>()?);
        cursor.seek(SeekFrom::Current(176 - (148 + 4)))?;
        let duration_ms = cursor.read_u32::<LittleEndian>()?; // milliseconds
        let cloud_catalog_album_id = cursor.read_u32::<LittleEndian>()?;
        let cloud_catalog_album_id = core::num::NonZeroU32::new(cloud_catalog_album_id);
        let cloud_catalog_album_id = cloud_catalog_album_id.map(|id| unsafe { crate::id::cloud::Catalog::new_unchecked(id) });
        cursor.seek(SeekFrom::Current(4))?;
        let cloud_catalog_artist_id = cursor.read_u32::<LittleEndian>()?;
        let cloud_catalog_artist_id = core::num::NonZeroU32::new(cloud_catalog_artist_id);
        let cloud_catalog_artist_id = cloud_catalog_artist_id.map(|id| unsafe { crate::id::cloud::Catalog::new_unchecked(id) }); 
        cursor.seek(SeekFrom::Current(316 - (176 + 16)))?;
        let bytes = cursor.read_u32::<LittleEndian>()?;
        cursor.seek(SeekFrom::Current(4))?;
        let cloud_catalog_track_id = cursor.read_u32::<LittleEndian>()?;
        let cloud_catalog_track_id = core::num::NonZeroU32::new(cloud_catalog_track_id);
        let cloud_catalog_track_id = cloud_catalog_track_id.map(|id| unsafe { crate::id::cloud::Catalog::new_unchecked(id) });
        // it also appears no less than three fuckin times after this appearance so like
        // uh. need to investigate that

        cursor.seek(SeekFrom::Current((length as i64) - (316 + 12)))?;

        Ok(Self {
            bitrate,
            date_added,
            date_modified,
            duration_ms,
            bytes,

            cloud_catalog_album_id,
            cloud_catalog_artist_id,
            cloud_catalog_track_id
        })
    }

    /// The duration of the track as a [`core::time::Duration`].
    pub fn duration(&self) -> core::time::Duration {
        core::time::Duration::from_millis(self.duration_ms as u64)
    }

    /// A link to the track through Apple Music, if it can be made.
    // TODO: Store regions? Names in the URL?
    pub fn apple_music_url(&self) -> Option<String> {
        if let Some(album_id) = self.cloud_catalog_album_id {
            if let Some(track_id) = self.cloud_catalog_track_id {
                return Some(format!("https://music.apple.com/album/{album_id}?i={track_id}"))
            }
        }
        None
    }
}

#[derive(Default, Debug)]
pub struct TrackPlayStatistics {
    pub last: Option<chrono::DateTime<chrono::Utc>>,
    pub times: u32,
}
impl TrackPlayStatistics {
    pub const BOMA_SUBTYPE: u32 = 0x17;

    pub const fn never() -> Self {
        Self {
            last: None,
            times: 0
        }
    }

    pub fn read_content(cursor: &mut super::chunk::ChunkCursor<'_>, length: u32) -> Result<Self, std::io::Error> {
        cursor.seek(SeekFrom::Current(4))?; // padding
        cursor.seek(SeekFrom::Current(8))?; // skip repeat of track ID
        let last = convert_timestamp(cursor.read_u32::<LittleEndian>()?);
        let times = cursor.read_u32::<LittleEndian>()?;
        cursor.seek(SeekFrom::Current(length as i64 - 36))?;
        Ok(Self { last, times })
    }
}

#[derive(Debug)]
pub struct UnknownBoma<'a> {
    // r0x0..3 ; b"boma"
    // r0x4..7 ; ??
    // r0x8..11 ; len
    pub subtype: u32, // r0x12..15,
    pub bytes: &'a [u8],
}
impl<'a> UnknownBoma<'a> {
    pub fn read_variant_content(cursor: &mut super::chunk::ChunkCursor<'a>, length: u32, subtype: u32) -> Result<Self, std::io::Error> {
        Ok(Self { subtype, bytes: cursor.read_slice_exact((length as usize) - 16)? })
    }

    pub fn as_utf16le(&self) -> Result<&Utf16Str, unaligned_u16::utf16::InvalidUtf16Error> {
        Utf16Str::new(self.bytes)
    }
}

#[derive(strum_macros::FromRepr, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u32)]
pub enum BomaUtf16Variant {
    TrackTitle = 0x2,
    Album = 0x3,
    Artist = 0x4,
    Genre = 0x5,
    Kind = 0x6,
    Equalizer = 0x7, // TODO: Map to enum from weird storage form of "#!#123#!#"
    Comment = 0x8,
    // Observed on a track with a bunch of spammy metadata pointing to a site.
    // I don't have access to the actual track file, so I can't mess around to see what it's supposed to be.
    TrackFieldUnknown0 = 0x9,
    Composer = 0xC,
    Grouping = 0xE,
    TrackFieldUnknown1 = 0x12, // String of numbers with leading \0\0\0\0; https://promoonly.com/
    TrackFieldUnknown2 = 0x16, // Appeared alongside TrackFieldUnknown0
    TrackFieldUnknown3 = 0x34, // Right after Fairplay, "2:256"
    AlbumArtist = 0x1B,
    
    ClassicalWorkName = 63,
    ClassicalMovementTitle = 64,

    FairPlayInfo = 43,

    SortOrderTrackTitle = 0x1E,
    SortOrderAlbum = 0x1F,
    SortOrderArtist = 0x20,
    SortOrderAlbumArtist = 0x21,
    SortOrderComposer = 0x22,

    // CopyrightHolder = 0x2B,
    CopyrightHolder = 0x2E,
    
    TrackLocalFilePath = 67,

    PurchaserEmail = 0x3B,
    PurchaserName = 0x3C,


    PlaylistName = 200,

    IamaAlbum = 0x12C,
    IamaAlbumArtist = 0x12D,
    IamaAlbumArtistCloud = 0x12E, // not on local albums (maybe it would if it was a recognized music / available on apple music?)
    SeriesTitle = 0x12F,
    IamaAlbumCloudId = 0x133,

    ArtistsArtistName = 400,
    ArtistsArtistNameSorted = 401, // for use in sorted lists, e.x. without leading "The"
    ArtistsArtistCloudLibraryId = 403,

    AccountCloudId = 800, // `sp.{UUIDv4}`
    AccountDisplayName = 801,
    AccountUsername = 802,
    AccountUrlSafeId = 803, // used for album cover URL
    AccountAvatarUrl = 804,



    UnknownHex1 = 0x1F4,
    ManagedMediaFolder = 0x1F8,
    UnknownHex2 = 0x1FE,
}
impl BomaUtf16Variant {
    pub const fn is_recognized_unknown(self) -> bool {
        matches!(self,
              Self::TrackFieldUnknown0
            | Self::TrackFieldUnknown1
            | Self::TrackFieldUnknown2
            | Self::TrackFieldUnknown3
            | Self::UnknownHex1
            | Self::UnknownHex2
        )
    }
}

#[derive(thiserror::Error, Debug)]
pub enum BomaUtf16Error<'a> {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid utf-16 string: {0}")]
    InvalidUtf16(unaligned_u16::utf16::InvalidUtf16Error, &'a [u8])
}


#[derive(Debug)]
pub struct BomaUtf16<'a>(pub &'a Utf16Str, pub BomaUtf16Variant);
impl<'a> BomaUtf16<'a> {
    fn read_variant_content(cursor: &mut super::chunk::ChunkCursor<'a>, variant: BomaUtf16Variant) -> Result<Self, BomaUtf16Error<'a>> {
        // r = 0x12 ; have read shared header
        // but we also skip unknown in struct which is also 12 bytes
        cursor.advance(8)?;
        let byte_length = cursor.read_u32::<LittleEndian>()? as usize;
        cursor.advance(8)?;
        let slice: &[u8] = cursor.read_slice_exact(byte_length)?;
        let str = Utf16Str::new(slice).map_err(|err| BomaUtf16Error::InvalidUtf16(err, slice))?;
        Ok(Self(str, variant))
    }
}

#[derive(strum_macros::FromRepr, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u32)]
pub enum BomaUtf8Variant {
    PlistTrackCloudInformation = 0x36,
    PlistCloudDownloadInformation = 0x38,
    PlistArtworkURL = 0x192,
    PlistPlaylistInfo = 0xCD,
    PlistAssetInfo = 0x1D,
    TrackLocalFilePathUrl = 0xB,
    // Observed on a track with a bunch of spammy metadata pointing to a site.
    // I don't have access to the actual track file, so I can't mess around to see what it's supposed to be.
    TrackFieldUnknown0 = 0x13,
    // Observed on a track spamming in all the metadata about how it's for promotional use only.
    // I don't have access to the actual track file, so I can't mess around to see
    TrackFieldUnknown1 = 0x25,
}
impl BomaUtf8Variant {
    pub const fn is_recognized_unknown(self) -> bool {
        matches!(self,
              Self::TrackFieldUnknown0
            | Self::TrackFieldUnknown1
        )
    }
}

#[derive(Debug)]
pub struct BomaUtf8<'a>(pub &'a str, pub BomaUtf8Variant);
impl<'a> BomaUtf8<'a> {
    pub(crate) fn read_variant_content(cursor: &mut super::chunk::ChunkCursor<'a>, mut length: u32, variant: BomaUtf8Variant) -> Result<Self, std::io::Error> {
        cursor.advance(4)?;

        // awesome.
        if variant == BomaUtf8Variant::TrackLocalFilePathUrl {
            cursor.advance(16)?;
            length -= 16;
        }

        let slice = cursor.read_slice((length as usize) - 20);
        Ok(Self(unsafe { str::from_utf8_unchecked(slice) }, variant))
    }
}


#[derive(serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PlistTrackCloudInformation<'a> {
    cloud_album_id: &'a str,
    cloud_artist_id: &'a str,
    cloud_artwork_token: &'a str
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PlistTrackCloudDownloadInformation<'a> {
    cloud_universal_library_id: &'a str,
    redownload_params: &'a str
}

#[derive(strum_macros::FromRepr, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u32)]
pub enum BookVariant {
    Variant0 = 0x42,
    // Variant1 = 0x1FC,
    Variant2 = 0x1FD,
    // Variant3 = 0x200
}

#[derive(Debug)]
pub enum BookValue<'a> {
    Binary(&'a [u8]),
    String(&'a str)
}


// enum Indicator {
//     PathComponent = 257,
//     FileProtocol = 2305, // always "file:///" ; but that final slash maybe means "start of file URL?" cuz :// is protocol so the last slash is root dir
//     // 513 - hex + ?? + sandbox info + path
//     // 772 - decently consistent raw
//     //  - last 772 first byte seemingly correlates a tad bit with song index ?
// }

#[derive(Debug)]
pub struct BomaBook<'a>(Vec<BookValue<'a>>, BookVariant);
impl<'a> BomaBook<'a> {
    pub(crate) fn read_variant_content(cursor: &mut super::chunk::ChunkCursor<'a>, length: u32, variant: BookVariant) -> Result<Self, std::io::Error> {
        assert_eq!(cursor.read_slice_exact(4)?, b"\0\0\0\0", "expected null padding");
        let signature = cursor.read_signature()?;

        if signature != *b"book" {
            // In some cases (on windows?), this boma actually just jumps into a UTF16 path string or other junk.
            // Eventually we should pass context (apple music version, etc) to the reading function to conditionally handle that.
            #[cfg(feature = "tracing")]
            tracing::warn!(?variant, "did not encounter expected book subtype signature");
            cursor.advance(length as i64 - 24)?;
            return Ok(Self(vec![], variant))
        }

        assert_eq!(&signature, b"book");
        let mut values = vec![];
        let destination = cursor.position() - 24 + length as usize;
        cursor.advance(48)?;
        while cursor.position() != destination {
            let length = cursor.read_u32::<LittleEndian>()? as usize;
            let indicator = cursor.read_u32::<LittleEndian>()?; // ?
            let slice = cursor.read_slice_exact(length)?;
            let padding = -((length % 4) as i64) & 3; // align to 4 bytes, moving 0-3
            cursor.advance(padding);

            let has_two_sequential_zeros = slice.windows(2).filter(|v| v == &[0, 0]).take(1).count() == 1;

            let value = if has_two_sequential_zeros {
                BookValue::Binary(slice)
            } else {
                match std::str::from_utf8(slice) {
                    Ok(string) => BookValue::String(string),
                    Err(_) => BookValue::Binary(slice)
                }
            };

            values.push(value);
        }

        Ok(Self(values, variant))     
    }
}
