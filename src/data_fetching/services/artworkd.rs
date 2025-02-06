#[allow(unused)]
use std::ffi::OsStr;
use std::{borrow::Borrow, cell::RefCell, sync::Mutex};

use apple_music::Track;
use maybe_owned_string::MaybeOwnedString;
use rusqlite::{params, Connection, OpenFlags, Result};
use tokio::sync::RwLock;

struct PersistentId(i64);
impl TryFrom<&str> for PersistentId {
    type Error = core::num::ParseIntError;
    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        let bytes = u64::from_str_radix(value, 16)?.to_ne_bytes();
        let signed = i64::from_ne_bytes(bytes);
        Ok(PersistentId(signed))
    }
}

#[derive(strum::FromRepr, PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
enum Kind {
    UserCustomAlbumArt = 6,
    UnknownAlbumArt1 = 12, // ?
    UnknownAlbumArt2 = 13, // ?
    UnknownAlbumArt3 = 17, // ??
    AuthorAvatar = 45, // Sometimes automatically generated, sometimes not.
    PlaylistCover = 63,
}


struct SourceInfo {
    pub url: Option<String>,
    pub fk_image_info: Option<ImageInfoKey>
}

fn get_source_info(persistent_id: PersistentId, connection: &mut Connection) -> Result<SourceInfo, rusqlite::Error> {
    let mut prepared  = connection.prepare_cached(r"
        SELECT ZIMAGEINFO, ZURL
        FROM ZDATABASEITEMINFO AS db
        JOIN ZSOURCEINFO AS src
        ON db.ZSOURCEINFO = src.Z_PK
        WHERE db.ZPERSISTENTID = ?1;
    ")?;

    prepared.query_row([persistent_id.0], |out| {
        Ok(SourceInfo {
            url: out.get(1)?,
            fk_image_info: out.get::<usize, Option<usize>>(0)?.map(ImageInfoKey)
        })
    })
}

fn as_optional<T>(result: Result<T, rusqlite::Error>) -> Result<Option<T>, rusqlite::Error> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(err) => Err(err)
    }
}

struct ImageInfoKey(usize);
struct ImageInfo {
    pub hash_string: String,
    pub kind: Kind,
}

fn get_image_info(key: ImageInfoKey, connection: &mut Connection) -> Result<Option<ImageInfo>, rusqlite::Error> {
    let mut prepared  = connection.prepare_cached(r"
        SELECT ZHASHSTRING, ZKIND
        FROM ZIMAGEINFO
        WHERE Z_PK = ?1;
    ")?;

    as_optional(prepared.query_row([key.0], |out| {
        Ok(ImageInfo {
            kind: Kind::from_repr(out.get::<usize, u8>(1)?).expect("unknown variant"),
            hash_string: out.get(0)?,
        })
    }))
}

#[derive(Debug)]
pub enum StoredArtwork {
    Remote { url: String },
    Local { path: String }
}

use std::sync::LazyLock;

use super::custom_artwork_host::CustomArtworkHost;
static ARTWORKD_PATH: LazyLock<std::path::PathBuf> = LazyLock::new(|| {
    crate::util::HOME.as_path().join("Library/Containers/com.apple.AMPArtworkAgent/Data/Documents")
});

// Merely knowing this function exists has brought me great pain, to say much less of writing it.
// One day I hope it may be rendered unnecessary. 
fn get_file_extension(info: &ImageInfo) -> String {
    if info.kind == Kind::UserCustomAlbumArt {
        let folder = ARTWORKD_PATH.join("artwork");
        let folder = folder.to_str().expect("bad album artwork path");

        for file in std::fs::read_dir(folder).expect("cannot read album art folder") {
            let file = file.expect("cannot read album art file").file_name();
            if file.to_string_lossy().starts_with(&info.hash_string) {
                return std::path::Path::new(&file).extension().expect("file has no extension").to_string_lossy().to_string()
            }
        }
    };

    "jpeg".to_string()
}

/// ## Parameters
/// - `persistent_id`: Hexadecimal string containing 8 bytes.
pub fn get_artwork(persistent_id: impl AsRef<str>) -> Result<Option<StoredArtwork>, rusqlite::Error> {
    let mut connection = Connection::open_with_flags(ARTWORKD_PATH.join("artworkd.sqlite"), OpenFlags::SQLITE_OPEN_READ_ONLY).expect("cannot connect to artworkd database");
    let persistent_id = PersistentId::try_from(persistent_id.as_ref()).expect("bad persistent ID");
    let source = get_source_info(persistent_id, &mut connection)?;
    if let Some(url) = source.url { return Ok(Some(StoredArtwork::Remote { url })) }
    if let Some(fk) = source.fk_image_info {
        return match get_image_info(fk, &mut connection)? {
            None => Ok(None),
            Some(info) => {
                const CACHE_ID: usize = 1; // it's kinda borked
                Ok(Some(StoredArtwork::Local {
                    path: format!("{}/{}_sk_{}_cid_{}.{}",
                        ARTWORKD_PATH.join("artwork").as_path().display(),
                        info.hash_string,
                        info.kind as u8,
                        CACHE_ID,
                        get_file_extension(&info)
                    )
                }))
            }
        }
    }
    Ok(None)
}
