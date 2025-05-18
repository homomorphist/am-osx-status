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

async fn get_source_info(persistent_id: PersistentId, pool: &sqlx::SqlitePool) -> Result<Option<SourceInfo>, sqlx::Error> {
    sqlx::query(r"
        SELECT ZIMAGEINFO, ZURL
        FROM ZDATABASEITEMINFO AS db
        JOIN ZSOURCEINFO AS src
        ON db.ZSOURCEINFO = src.Z_PK
        WHERE db.ZPERSISTENTID = ?1;
    ")
        .bind(persistent_id.0)
        .fetch_optional(pool).await?
        .map(|row| {
            use sqlx::Row;
            Ok(SourceInfo {
                url: row.get(1),
                fk_image_info: row.get::<Option<u64>, usize>(0).map(ImageInfoKey)
            })
        })
        .transpose()
}


struct ImageInfoKey(u64);
struct ImageInfo {
    pub hash_string: String,
    pub kind: Kind,
}

async fn get_image_info(key: ImageInfoKey, pool: &sqlx::SqlitePool) -> Result<Option<ImageInfo>, sqlx::Error> {
    sqlx::query(r"
        SELECT ZHASHSTRING, ZKIND
        FROM ZIMAGEINFO
        WHERE Z_PK = ?1;
    ")
        .bind(key.0 as i64)
        .fetch_optional(pool).await?
        .map(|out| {
            use sqlx::Row;
            Ok(ImageInfo {
                kind: Kind::from_repr(out.get(1)).expect("unknown variant"),
                hash_string: out.get::<&str, usize>(0).to_string(),
            })
        })
        .transpose()
}

#[derive(Debug)]
pub enum StoredArtwork {
    Remote { url: String },
    Local { path: String }
}

use std::{ops::DerefMut, sync::LazyLock};

use sqlx::{pool, Connection};
static ARTWORKD_PATH: LazyLock<std::path::PathBuf> = LazyLock::new(|| {
    crate::util::HOME.as_path().join("Library/Containers/com.apple.AMPArtworkAgent/Data/Documents")
});
static ARTWORKD_ARTWORK_PATH: LazyLock<std::path::PathBuf> = LazyLock::new(|| {
    ARTWORKD_PATH.as_path().join("artwork")
});
static ARTWORKD_SQLITE_PATH: LazyLock<std::path::PathBuf> = LazyLock::new(|| {
    ARTWORKD_PATH.as_path().join("artworkd.sqlite")
});

// Merely knowing this function exists has brought me great pain, to say much less of writing it.
// One day I hope it may be rendered unnecessary. 
fn get_file_extension(info: &ImageInfo) -> String {
    if info.kind == Kind::UserCustomAlbumArt {
        let folder = &*ARTWORKD_ARTWORK_PATH;
        for file in std::fs::read_dir(folder).expect("cannot read album art folder") {
            let file = file.expect("cannot read album art file").file_name();
            if file.to_string_lossy().starts_with(&info.hash_string) {
                return std::path::Path::new(&file).extension().expect("file has no extension").to_string_lossy().to_string()
            }
        }
    };

    "jpeg".to_string()
}


static POOL: crate::store::GlobalPool = crate::store::GlobalPool::new(|| {
    crate::store::GlobalPoolOptions {
        pool: sqlx::sqlite::SqlitePoolOptions::new(),
        connect: sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&*ARTWORKD_SQLITE_PATH)
            .read_only(true),
    }
});



/// ## Parameters
/// - `persistent_id`: Hexadecimal string containing 8 bytes.
pub async fn get_artwork(persistent_id: impl AsRef<str>) -> Result<Option<StoredArtwork>, crate::store::MaybeStaticSqlError> {
    let pool = POOL.get().await?;
    let persistent_id = PersistentId::try_from(persistent_id.as_ref()).expect("bad persistent ID");
    let source = get_source_info(persistent_id, &pool).await?;
    let source = if let Some(source) = source { source } else { return Ok(None) };
    if let Some(url) = source.url { return Ok(Some(StoredArtwork::Remote { url })) }
    if let Some(fk) = source.fk_image_info {
        return match get_image_info(fk, &pool).await? {
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
