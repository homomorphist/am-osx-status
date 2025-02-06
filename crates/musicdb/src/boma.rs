#![allow(unused)]
use core::str;
use std::io::{Cursor, Read, Seek, SeekFrom};


use byteorder::{LittleEndian, ReadBytesExt};

use crate::utf16::Utf16Str;
use crate::version::AppleMusicVersion;
use crate::{CollectionMember, Reader};

use super::{convert_timestamp, ContextlessRead};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnknownBomaError(pub u32);
impl std::error::Error for UnknownBomaError {}
impl core::fmt::Display for UnknownBomaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown boma subtype '{}'", self.0)
    }
}


#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BomaSubtype {
    TrackNumerics,
    CollectionItemMember,
    Book(BookVariant),
    Utf16(BomaUtf16Variant),
    Utf8Xml(BomaUtf8Variant),
}
impl BomaSubtype {
    pub fn get_raw(&self) -> u32 {
        match self {
            Self::TrackNumerics => TrackNumerics::BOMA_SUBTYPE,
            Self::CollectionItemMember => CollectionMember::BOMA_SUBTYPE,
            Self::Utf16(variant) => *variant as u32,
            Self::Utf8Xml(variant) => *variant as u32,
            Self::Book(variant) => *variant as u32,
        }
    }
}
impl TryFrom<u32> for BomaSubtype {
    type Error = UnknownBomaError;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        if value == TrackNumerics::BOMA_SUBTYPE {
            return Ok(Self::TrackNumerics)
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
    TrackNumerics(TrackNumerics),
    CollectionMember(CollectionMember<'a>),
    Utf16(BomaUtf16<'a>),
    Utf8Xml(BomaUtf8<'a>),
    Book(BomaBook<'a>),
    Unknown(UnknownBoma)
}
impl<'a> ContextlessRead<'a> for Boma<'a> {
    type ReadError = std::io::Error;
    const SIGNATURE: &'static [u8; 4] = b"boma";

    fn read_contents(mut reader: &mut Reader<'a>) -> Result<Self, Self::ReadError> where Self: Sized {
        reader.advance(4)?;
        let length = reader.cursor.read_u32::<LittleEndian>()?;
        let subtype = reader.cursor.read_u32::<LittleEndian>()?;

        Ok(match BomaSubtype::try_from(subtype) {
            Ok(subtype) => match subtype {
                BomaSubtype::TrackNumerics => Self::TrackNumerics(TrackNumerics::read_content(&mut reader.cursor, length)?),
                BomaSubtype::CollectionItemMember => Self::CollectionMember(CollectionMember::read_content(reader)?),
                BomaSubtype::Utf16(variant) => Self::Utf16(BomaUtf16::read_variant_content(reader, variant).expect("please handle error")),
                BomaSubtype::Utf8Xml(variant) => Self::Utf8Xml(BomaUtf8::read_variant_content(reader, length, variant)?),
                BomaSubtype::Book(variant) => Self::Book(BomaBook::read_variant_content(reader, length, variant)?)
            },
            Err(UnknownBomaError(subtype)) => Self::Unknown(UnknownBoma::read_variant_content(&mut reader.cursor, length, subtype)?)
        })
    }
}
impl Boma<'_> {
    pub fn get_subtype(&self) -> Result<BomaSubtype, UnknownBomaError> {
        match self {
            Self::TrackNumerics(_) => Ok(BomaSubtype::TrackNumerics),
            Self::CollectionMember(_) => Ok(BomaSubtype::CollectionItemMember),
            Self::Utf16(BomaUtf16(_, variant)) => Ok(BomaSubtype::Utf16(*variant)),
            Self::Utf8Xml(BomaUtf8(_, variant)) => Ok(BomaSubtype::Utf8Xml(*variant)),
            Self::Book(BomaBook(_, variant)) => Ok(BomaSubtype::Book(*variant)),
            Self::Unknown(UnknownBoma { subtype, .. }) => Err(UnknownBomaError(*subtype))
        }
    }
}

#[derive(Debug)]
pub struct TrackNumerics {
    pub bitrate: Option<crate::units::KilobitsPerSecond>,
    date_added: Option<chrono::DateTime<chrono::Utc>>,
    date_modified: Option<chrono::DateTime<chrono::Utc>>,
    /// Duration of the track, in milliseconds.
    pub duration_ms: u32,
    /// File size, in bytes.
    pub bytes: u32,
}

impl TrackNumerics {
    pub const BOMA_SUBTYPE: u32 = 0x1;

    pub fn read_content(cursor: &mut Cursor<&[u8]>, length: u32) -> Result<Self, std::io::Error> {
        cursor.seek(SeekFrom::Current(108 - (12 + 4)))?;
        let bitrate = cursor.read_u32::<LittleEndian>()?;
        let bitrate = if bitrate == 0 { None } else { Some(crate::units::KilobitsPerSecond(bitrate)) };
        let date_added = convert_timestamp(cursor.read_u32::<LittleEndian>()?);
        cursor.seek(SeekFrom::Current(148 - (112 + 4)))?;
        let date_modified = convert_timestamp(cursor.read_u32::<LittleEndian>()?);
        cursor.seek(SeekFrom::Current(176 - (148 + 4)))?;
        let duration_ms = cursor.read_u32::<LittleEndian>()?; // milliseconds
        cursor.seek(SeekFrom::Current(316 - (176 + 4)))?;
        let bytes = cursor.read_u32::<LittleEndian>()?;
        cursor.seek(SeekFrom::Current((length as i64) - (316 + 4)))?;

        Ok(Self {
            bitrate,
            date_added,
            date_modified,
            duration_ms,
            bytes
        })
    }

    /// Return the duration of the track in a [`core::time::Duration`].
    pub fn duration(&self) -> core::time::Duration {
        core::time::Duration::from_millis(self.duration_ms as u64)
    }
}

#[derive(Debug)]
pub struct UnknownBoma {
    // r0x0..3 ; b"boma"
    // r0x4..7 ; ??
    // r0x8..11 ; len
    subtype: u32, // r0x12..15,
    bytes: Vec<u8>,
}
impl UnknownBoma {
    pub fn read_variant_content(cursor: &mut Cursor<&[u8]>, length: u32, subtype: u32) -> Result<Self, std::io::Error> {
        let mut bytes = vec![0; (length as usize) - 16];
        cursor.read_exact(&mut bytes[..])?;
        Ok(Self { subtype, bytes })
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
    Comment = 0x8,
    Composer = 0xC,
    Grouping = 14,
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
    ArtistsArtistCloudId = 403,

    AccountCloudId = 800, // `sp.{UUIDv4}`
    AccountDisplayName = 801,
    AccountUsername = 802,
    AccountUrlSafeId = 803, // used for album cover URL
    AccountAvatarUrl = 804,



    UnknownHex1 = 0x1F4,
    ManagedMediaFolder = 0x1F8,
    UnknownHex2 = 0x1FE
}

#[derive(thiserror::Error, Debug)]
pub enum BomaUtf16Error<'a> {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid utf-16 string: {0}")]
    InvalidUtf16(crate::utf16::error::InvalidUtf16, &'a [u8])
}


#[derive(Debug)]
pub struct BomaUtf16<'a>(pub Utf16Str<'a>, pub BomaUtf16Variant);
impl<'a> BomaUtf16<'a> {
    fn read_variant_content(reader: &mut Reader<'a>, variant: BomaUtf16Variant) -> Result<Self, BomaUtf16Error<'a>> {
        // r = 0x12 ; have read shared header
        // but we also skip unknown in struct which is also 12 bytes
        reader.advance(8)?;
        let byte_length = reader.cursor.read_u32::<LittleEndian>()? as usize;
        reader.advance(8)?;
        let slice: &[u8] = reader.read_slice(byte_length)?;
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
    TrackLocalFilePathUrl = 11,
}

#[derive(Debug)]
pub struct BomaUtf8<'a>(pub &'a str, pub BomaUtf8Variant);
impl<'a> BomaUtf8<'a> {
    pub(crate) fn read_variant_content(reader: &mut Reader<'a>, mut length: u32, variant: BomaUtf8Variant) -> Result<Self, std::io::Error> {
        reader.advance(4)?;

        // awesome.
        if variant == BomaUtf8Variant::TrackLocalFilePathUrl {
            reader.advance(16)?;
            length -= 16;
        }

        let slice = reader.read_slice((length as usize) - 20)?;
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
    pub(crate) fn read_variant_content(reader: &mut Reader<'a>, length: u32, variant: BookVariant) -> Result<Self, std::io::Error> {
        const V5: AppleMusicVersion = AppleMusicVersion {
            major: 1,
            minor: 5,
            patch: 0,
            revision: 0,
        };

        if reader.version.unwrap() >= V5 && variant != BookVariant::Variant0 {
            // not a book, some other boma. TODO: fix
            reader.advance(length as i64 - 16)?;
            return Ok(Self(vec![], variant))
        }

        reader.advance(4)?;
        assert_eq!(&reader.read_signature(), b"book");
        let mut values = vec![];
        let destination = reader.cursor.position() - 24 + length as u64;
        reader.advance(48)?;
        while reader.cursor.position() != destination {
            let length = reader.cursor.read_u32::<LittleEndian>()? as usize;
            let indicator = reader.cursor.read_u32::<LittleEndian>()?; // ?
            let slice = reader.read_slice(length)?;
            let padding = -((length % 4) as i64) & 3; // align to 4 bytes, moving 0-3
            reader.advance(padding);

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
