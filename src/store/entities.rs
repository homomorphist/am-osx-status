use crate::status_backend::error::DispatchError;
use super::MaybeStaticSqlError;


pub struct Key<T>(i64, core::marker::PhantomData<T>);
impl<'r, T> sqlx::Encode<'r, sqlx::Sqlite> for Key<T> where i64: sqlx::Encode<'r, sqlx::Sqlite> {
    fn encode_by_ref(&self, buf: &mut <sqlx::Sqlite as sqlx::Database>::ArgumentBuffer<'r>) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + 'static + Send + Sync>> {
        <i64 as sqlx::Encode<sqlx::Sqlite>>::encode_by_ref(&self.0, buf)
    }
}
impl<'r, T,  DB: sqlx::Database> sqlx::Decode<'r, DB> for Key<T> where i64: sqlx::Decode<'r, DB> {
    fn decode(value: DB::ValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + 'static + Send + Sync>> {
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


trait KeyCollection<T>: Sized {
    fn len(&self) -> usize;
    fn as_slice(&self) -> &[Key<T>];
}
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
    const TABLE_NAME: &'static str = "deferred_track";
}
impl DeferredTrack {
    pub async fn insert_in_pool(pool: &sqlx::SqlitePool, track: &crate::DispatchableTrack) -> sqlx::Result<Key<Self>> {
        sqlx::query_as::<_, Self>(r#"
            INSERT INTO deferred_track (
                title,
                artist,
                album,
                album_artist,
                album_index,
                persistent_id,
                duration,
                media_kind
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id
        "#)
            .bind(&track.name)
            .bind(&track.artist)
            .bind(&track.album)
            .bind(&track.album_artist)
            .bind(track.track_number)
            .bind(&track.persistent_id)
            .bind(track.duration.map(|d| d.as_secs_f32() as f64))
            .bind(&track.media_kind)
            .fetch_one(pool).await
            .map(|v| v.id)
    }
    pub async fn insert(track: &crate::DispatchableTrack) -> Result<Key<Self>, super::MaybeStaticSqlError> {
        let pool = crate::store::DB_POOL.get().await?;
        Self::insert_in_pool(&pool, track).await.map_err(Into::into)
    }

    pub async fn get_with_persistent_id_in_pool(pool: &sqlx::SqlitePool, persistent_id: &str) -> sqlx::Result<Option<Self>> {
        sqlx::query_as::<_, Self>(r#"
            SELECT * FROM deferred_track WHERE persistent_id = ?
        "#)
            .bind(persistent_id)
            .fetch_optional(pool).await
    }
    pub async fn get_with_persistent_id(persistent_id: &str) -> Result<Option<Self>, super::MaybeStaticSqlError> {
        let pool = crate::store::DB_POOL.get().await?;
        Self::get_with_persistent_id_in_pool(&pool, persistent_id).await.map_err(Into::into)
    }
}

#[derive(sqlx::FromRow)]
pub struct Session {
    id: Key<Self>,

    /// The (semver) version of the crate this session was ran on.
    #[sqlx(rename = "ver_crate")]
    pub version: String,

    /// The version of apple music at the time of the session.
    /// This is like a semver, but it has four parts.
    #[sqlx(rename = "ver_music")]
    pub am_version: String,

    /// The (semver) version of the operating system at the time of the session.
    #[sqlx(rename = "ver_os")]
    pub os_version: String,

    pub osa_polls_track: i64,
    pub osa_polls_music: i64,

    pub started_at: chrono::DateTime<chrono::Utc>,
    pub ended_at: Option<chrono::DateTime<chrono::Utc>>,
}
impl Session {
    pub fn duration(&self) -> chrono::Duration {
        self.ended_at.unwrap_or_else(chrono::Utc::now) - self.started_at
    }
}
impl FromKey for Session {
    const TABLE_NAME: &'static str = "session";
}
impl Session {
    pub async fn new(
        pool: &sqlx::SqlitePool,
        am_version: &str,
        os_version: &str,
    ) -> sqlx::Result<Self> {
        sqlx::query_as::<_, Self>(r#"
            INSERT INTO session (
                ver_crate,
                ver_music,
                ver_os,
                started_at
            ) VALUES (?, ?, ?,?) RETURNING * 
        "#)
            .bind(clap::crate_version!())
            .bind(am_version)
            .bind(os_version)
            .bind(chrono::Utc::now())
            .fetch_one(pool).await
    }
    pub async fn finish(&self, pool: &sqlx::SqlitePool) -> sqlx::Result<()> {
        let now = chrono::Utc::now();
        sqlx::query!(r#"
            UPDATE session SET
                ended_at = ?,
                osa_polls_track = ?,
                osa_polls_music = ?
            WHERE id = ?
        "#, 
            now,
            self.osa_polls_track,
            self.osa_polls_music,
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
    timestamp: chrono::DateTime<chrono::Utc>,
    fmt_display: String,
    fmt_debug: String,
    session: Key<Session>,
}
impl FromKey for Error {
    const TABLE_NAME: &'static str = "errors";
}
impl Error {
    async fn new(pool: &sqlx::SqlitePool, session: &Session, source: &DispatchError) -> sqlx::Result<Self> {
        sqlx::query_as::<_, Self>(r#"
            INSERT INTO error (
                fmt_display,
                fmt_debug,
                session
            ) VALUES (?, ?, ?) RETURNING *
        "#)
            .bind(format!("{source}"))
            .bind(format!("{source:?}"))
            .bind(session.id)
            .fetch_one(pool).await
    }
}

#[derive(sqlx::FromRow)]
pub struct PendingDispatch {
    id: Key<Self>,
    timestamp: chrono::DateTime<chrono::Utc>,
    backend: String,
    #[sqlx(rename = "track")] track: Key<DeferredTrack>,
    #[sqlx(rename = "error")] error: Key<Error>,
}

#[cfg(test)]
mod tests {
    use super::super::test_utilities::*;

    #[test]
    fn session() {
        
    }
}
