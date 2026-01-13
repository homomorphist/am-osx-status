#![expect(dead_code, reason = "some stuff here is under construction")]

use crate::{store::types::{MillisecondTimestamp, StoredPersistentId}, subscribers::error::DispatchError};
use super::MaybeStaticSqlError;

pub struct Key<T>(i64, core::marker::PhantomData<T>);
impl<'r, T> sqlx::Encode<'r, sqlx::Sqlite> for Key<T> where i64: sqlx::Encode<'r, sqlx::Sqlite> {
    fn encode_by_ref(&self, buf: &mut <sqlx::Sqlite as sqlx::Database>::ArgumentBuffer<'r>) -> Result<sqlx::encode::IsNull, Box<dyn core::error::Error + 'static + Send + Sync>> {
        <i64 as sqlx::Encode<sqlx::Sqlite>>::encode_by_ref(&self.0, buf)
    }
}
impl<'r, T,  DB: sqlx::Database> sqlx::Decode<'r, DB> for Key<T> where i64: sqlx::Decode<'r, DB> {
    fn decode(value: DB::ValueRef<'r>) -> Result<Self, Box<dyn core::error::Error + 'static + Send + Sync>> {
        let value = <i64 as sqlx::Decode<DB>>::decode(value)?;
        Ok(Self(value, core::marker::PhantomData))
    }
}
impl<T> sqlx::Type<sqlx::Sqlite> for Key<T> where i64: sqlx::Type<sqlx::Sqlite> {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <i64 as sqlx::Type<sqlx::Sqlite>>::type_info()
    }
    fn compatible(ty: &sqlx::sqlite::SqliteTypeInfo) -> bool {
        <i64 as sqlx::Type<sqlx::Sqlite>>::compatible(ty)
    }
}
impl<T> From<i64> for Key<T> {
    fn from(value: i64) -> Self {
        Self(value, core::marker::PhantomData)
    }
}
impl<T> Clone for Key<T> {
    fn clone(&self) -> Self { *self }
}
impl<T> Copy for Key<T> {}
impl<T> core::fmt::Debug for Key<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Key::<{}>({})", core::any::type_name::<T>(), self.0)
    }
}

trait KeyCollection<T>: Sized {
    fn len(&self) -> usize;
    fn as_slice(&self) -> &[Key<T>];
}
#[expect(clippy::use_self, reason = "method overlap; this is clearer")]
impl<T> KeyCollection<T> for Vec<Key<T>> {
    fn len(&self) -> usize {
        Vec::len(self)
    }
    fn as_slice(&self) -> &[Key<T>] {
        Vec::as_slice(self)
    }
}
impl<T> KeyCollection<T> for &[Key<T>] {
    fn len(&self) -> usize {
        <[Key<T>]>::len(self)
    }
    fn as_slice(&self) -> &[Key<T>] {
        self
    }
}
impl<const N: usize, T> KeyCollection<T> for [Key<T>; N] {
    fn len(&self) -> usize {
        N
    }
    fn as_slice(&self) -> &[Key<T>] {
        self
    }
}

pub trait FromKey: Sized + for<'a> sqlx::FromRow<'a, sqlx::sqlite::SqliteRow> + Send + Unpin {
    const TABLE_NAME: &'static str;

    async fn get_in_pool(id: Key<Self>, pool: &sqlx::SqlitePool) -> Result<Self, MaybeStaticSqlError> {
        let session = sqlx::query_as::<_, Self>(format!("SELECT * FROM {} WHERE id = ?", Self::TABLE_NAME).as_str())
            .bind(id)
            .fetch_one(pool)
            .await?;
        Ok(session)
    }
    async fn get(id: Key<Self>) -> Result<Self, MaybeStaticSqlError> {
        let pool = crate::store::DB_POOL.get().await?;
        Self::get_in_pool(id, &pool).await
    }
    
    async fn get_many_in_pool(ids: impl AsRef<[Key<Self>]>, pool: &sqlx::SqlitePool) -> Result<Vec<Self>, MaybeStaticSqlError> {
        let ids = ids.as_ref();
        let query = format!(
            "SELECT * FROM {} WHERE id IN ({})",
            Self::TABLE_NAME,
            (0..ids.len()).map(|_| "?").collect::<Vec<_>>().join(",")
        );
        let mut session = sqlx::query_as::<_, Self>(query.as_str());
        for id in ids {
            session = session.bind(id);
        }
        Ok(session.fetch_all(pool).await?) 
    }
    async fn get_many(ids: impl AsRef<[Key<Self>]>) -> Result<Vec<Self>, MaybeStaticSqlError> {
        let pool = crate::store::DB_POOL.get().await?;
        Self::get_many_in_pool(ids, &pool).await
    }

    async fn has_in_pool(id: Key<Self>, pool: &sqlx::SqlitePool) -> Result<bool, MaybeStaticSqlError> {
        use sqlx::Row;
        let exists = sqlx::query(format!("SELECT EXISTS(SELECT 1 FROM {} WHERE id = ?)", Self::TABLE_NAME).as_str())
            .bind(id)
            .fetch_one(pool)
            .await?
            .get::<i64, _>(0) == 1;
        Ok(exists)
    }
    async fn has(id: Key<Self>) -> Result<bool, MaybeStaticSqlError> {
        let pool = crate::store::DB_POOL.get().await?;
        Self::has_in_pool(id, &pool).await
    }
}

#[derive(sqlx::FromRow)]
pub struct DeferredTrack {
    pub id: Key<Self>,
    #[sqlx(flatten)]
    pub track: crate::DispatchableTrack
}
impl FromKey for DeferredTrack {
    const TABLE_NAME: &'static str = "deferred_tracks";
}
impl DeferredTrack {
    pub async fn insert_in_pool(pool: &sqlx::SqlitePool, track: &crate::DispatchableTrack) -> sqlx::Result<Key<Self>> {
        sqlx::query_as::<_, Self>(r"
            INSERT INTO deferred_tracks (
                title,
                artist,
                album,
                album_artist,
                album_index,
                persistent_id,
                duration,
                media_kind
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id
        ")
            .bind(&track.name)
            .bind(&track.artist)
            .bind(&track.album)
            .bind(&track.album_artist)
            .bind(track.track_number)
            .bind(track.persistent_id)
            .bind(track.duration.map(|d| f64::from(d.as_secs_f32())))
            .bind(&track.media_kind)
            .fetch_one(pool).await
            .map(|v| v.id)
    }
    pub async fn insert(track: &crate::DispatchableTrack) -> Result<Key<Self>, super::MaybeStaticSqlError> {
        let pool = crate::store::DB_POOL.get().await?;
        Self::insert_in_pool(&pool, track).await.map_err(Into::into)
    }

    pub async fn get_with_persistent_id_in_pool(pool: &sqlx::SqlitePool, persistent_id: StoredPersistentId) -> sqlx::Result<Option<Self>> {
        sqlx::query_as::<_, Self>(r"
            SELECT * FROM deferred_tracks WHERE persistent_id = ?
        ")
            .bind(persistent_id)
            .fetch_optional(pool).await
    }
    pub async fn get_with_persistent_id(persistent_id: StoredPersistentId) -> Result<Option<Self>, super::MaybeStaticSqlError> {
        let pool = crate::store::DB_POOL.get().await?;
        Self::get_with_persistent_id_in_pool(&pool, persistent_id).await.map_err(Into::into)
    }
}

#[derive(sqlx::FromRow, Debug)]
pub struct Session {
    id: Key<Self>,

    /// The (semver) version of the crate this session was ran on.
    #[sqlx(rename = "ver_crate")]
    pub version: String,

    /// The version of apple music at the time of the session.
    /// This is like a semver, but it has four parts.
    #[sqlx(rename = "ver_player")]
    pub player_version: String,

    /// The (semver) version of the operating system at the time of the session.
    #[sqlx(rename = "ver_os")]
    pub os_version: String,

    /// JXA fetch count for track information.
    /// A positive integer.
    pub osa_fetches_track: i64,

    /// JXA fetch count for player information.
    /// A positive integer.
    pub osa_fetches_player: i64,

    pub started_at: MillisecondTimestamp,
    pub ended_at: Option<MillisecondTimestamp>,
}
impl Session {
    pub fn duration(&self) -> chrono::Duration {
        self.ended_at.map_or_else(chrono::Utc::now, |v| v.0) - self.started_at.0
    }
}
impl FromKey for Session {
    const TABLE_NAME: &'static str = "sessions";
}
impl Session {
    pub async fn new(
        player_version: &str,
        migration_id: super::migrations::MigrationID
    ) -> sqlx::Result<Self> {
        sqlx::query_as::<_, Self>(r"
            INSERT INTO sessions (
                ver_crate,
                ver_player,
                ver_os,
                migration_id
            ) VALUES (?, ?, ?, ?) RETURNING * 
        ")
            .bind(clap::crate_version!())
            .bind(player_version)
            .bind(crate::util::get_macos_version().await)
            .bind(migration_id)
            .fetch_one(&crate::store::DB_POOL.get().await.expect("couldn't get db pool")).await
    }
    pub async fn update(&self, pool: &sqlx::SqlitePool) -> sqlx::Result<()> {
        sqlx::query!(r#"
            UPDATE sessions SET
                osa_fetches_track = ?,
                osa_fetches_player = ?
            WHERE id = ?
        "#, 
            self.osa_fetches_track,
            self.osa_fetches_player,
            self.id
        ).execute(pool).await?;
        Ok(())
    }
    pub async fn finish(&self, pool: &sqlx::SqlitePool) -> sqlx::Result<()> {
        let now = chrono::Utc::now().timestamp_millis();
        sqlx::query!(r#"
            UPDATE sessions SET
                ended_at = ?,
                osa_fetches_track = ?,
                osa_fetches_player = ?
            WHERE id = ?
        "#, 
            now,
            self.osa_fetches_track,
            self.osa_fetches_player,
            self.id,
        ).execute(pool).await.and_then(|v| {
            if v.rows_affected() == 0 {
                Err(sqlx::Error::RowNotFound)
            } else {
                Ok(())
            }
        })
    }
}

#[derive(sqlx::FromRow)]
pub struct Error {
    id: Key<Self>,
    timestamp: MillisecondTimestamp,
    fmt_display: String,
    fmt_debug: String,
    session: Key<Session>,
}
impl FromKey for Error {
    const TABLE_NAME: &'static str = "errors";
}
impl Error {
    async fn new(pool: &sqlx::SqlitePool, session: &Session, source: &DispatchError) -> sqlx::Result<Self> {
        sqlx::query_as::<_, Self>(r"
            INSERT INTO errors (
                fmt_display,
                fmt_debug,
                session
            ) VALUES (?, ?, ?) RETURNING *
        ")
            .bind(format!("{source}"))
            .bind(format!("{source:?}"))
            .bind(session.id)
            .fetch_one(pool).await
    }
}

#[derive(sqlx::FromRow)]
pub struct PendingDispatch {
    id: Key<Self>,
    timestamp: MillisecondTimestamp,
    backend: String,
    #[sqlx(rename = "track")] track: Key<DeferredTrack>,
    #[sqlx(rename = "error")] error: Key<Error>,
}
impl PendingDispatch {
    pub async fn track(&self) -> DeferredTrack {
        DeferredTrack::get(self.track).await.expect("failed to get deferred track")
    }
    pub async fn error(&self) -> Error {
        Error::get(self.error).await.expect("failed to get error")
    }
}


#[derive(Debug, sqlx::FromRow)]
pub struct CustomArtworkUrl {
    id: Key<Self>,
    pub expires_at: Option<MillisecondTimestamp>,
    pub source_path: String,
    #[sqlx(rename = "artwork_url")]
    pub url: String,
}
impl FromKey for CustomArtworkUrl {
    const TABLE_NAME: &'static str = "custom_artwork_urls";
}
impl CustomArtworkUrl {
    pub async fn new(
        pool: &sqlx::SqlitePool,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
        source_path: &str,
        artwork_url: &str,
    ) -> sqlx::Result<Self> {
        let expires_at = expires_at.map(|dt| dt.timestamp_millis());
        sqlx::query_as::<_, Self>(r"
            INSERT INTO custom_artwork_urls (
                expires_at,
                source_path,
                artwork_url
            ) VALUES (?, ?, ?) RETURNING *
        ")
            .bind(expires_at)
            .bind(source_path)
            .bind(artwork_url)
            .fetch_one(pool).await
    }

    pub async fn get_by_source_path_in_pool(
        pool: &sqlx::SqlitePool,
        source_path: &str,
    ) -> sqlx::Result<Option<Self>> {
        sqlx::query_as::<_, Self>(r"
            SELECT * FROM custom_artwork_urls WHERE source_path = ?
        ")
            .bind(source_path)
            .fetch_optional(pool).await
    }
    
    // TODO: Run this on application startup as well, or every few hours.
    pub async fn cleanup(pool: &sqlx::SqlitePool) -> sqlx::Result<()> {
        let now = chrono::Utc::now().timestamp_millis();
        sqlx::query!(r#"
            DELETE FROM custom_artwork_urls WHERE expires_at < ?
        "#, now)
            .execute(pool).await?;
        Ok(())
    }
}
impl CustomArtworkUrl {
    pub fn is_expired(&self) -> bool {
        /// We account for network latency when checking expiration,
        /// since we'll need to tell Discord to themselves fetch the image.
        const LATENCY_OFFSET: core::time::Duration = core::time::Duration::from_secs(5);
        self.expires_at.is_some_and(|expires_at| expires_at < chrono::Utc::now() + LATENCY_OFFSET)
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct CachedFirstArtist {
    id: Key<Self>,
    pub persistent_id: StoredPersistentId,
    /// All artists for the track, verbatim.
    /// If this doesn't match, we know the track metadata changed and we should recompute.
    pub artists: String,
    /// The first artist for the track.
    pub artist: String,
}
impl FromKey for CachedFirstArtist {
    const TABLE_NAME: &'static str = "first_artists";
}
impl CachedFirstArtist {
    pub async fn new(
        pool: &sqlx::SqlitePool,
        persistent_id: StoredPersistentId,
        artists: &str,
        artist: &str,
    ) -> sqlx::Result<Self> {
        sqlx::query_as::<_, Self>(r"
            INSERT INTO first_artists (
                persistent_id,
                artists,
                artist
            ) VALUES (?, ?, ?) RETURNING *
        ")
            .bind(persistent_id)
            .bind(artists)
            .bind(artist)
            .fetch_one(pool).await
    }

    /// Deletes the entry with the given ID.
    /// Returns whether an entry was removed.
    async fn remove_by_id(
        pool: &sqlx::SqlitePool,
        id: Key<Self>,
    ) -> sqlx::Result<bool> {
        sqlx::query!("DELETE FROM first_artists WHERE id = ?", id)
            .execute(pool).await
            .map(|result| result.rows_affected() != 0)
    } 


    pub async fn get_by_persistent_id(
        pool: &sqlx::SqlitePool,
        persistent_id: StoredPersistentId,
        artists: &str,
    ) -> sqlx::Result<Option<Self>> {
        let got = sqlx::query_as::<_, Self>(r"
            SELECT * FROM first_artists WHERE persistent_id = ?
        ")
            .bind(persistent_id)
            .fetch_optional(pool).await;

        if let Ok(Some(got)) = &got && got.artists != artists {
            // data mismatch, will need to be recomputed
            Self::remove_by_id(pool, got.id).await?;
            return Ok(None);
        }

        got
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct CachedUncensoredTitle {
    id: Key<Self>,
    pub persistent_id: StoredPersistentId,
    pub uncensored: Option<String>,
    pub timestamp: MillisecondTimestamp,
}
impl FromKey for CachedUncensoredTitle {
    const TABLE_NAME: &'static str = "uncensored_titles";
}
impl CachedUncensoredTitle {
    pub async fn new(
        pool: &sqlx::SqlitePool,
        persistent_id: StoredPersistentId,
        uncensored: Option<&str>,
    ) -> sqlx::Result<Self> {
        sqlx::query_as::<_, Self>(r"
            INSERT INTO uncensored_titles (
                persistent_id,
                uncensored
            ) VALUES (?, ?) RETURNING *
        ")
            .bind(persistent_id)
            .bind(uncensored)
            .fetch_one(pool).await
    }

    pub async fn get_by_persistent_id(
        pool: &sqlx::SqlitePool,
        persistent_id: StoredPersistentId,
    ) -> sqlx::Result<Option<Self>> {
        sqlx::query_as::<_, Self>("SELECT * FROM uncensored_titles WHERE persistent_id = ?")
            .bind(persistent_id)
            .fetch_optional(pool).await
    }
}

