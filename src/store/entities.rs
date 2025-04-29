pub type DeferredTrack = crate::DispatchableTrack;

#[derive(PartialEq)]
struct Key<T>(i64, core::marker::PhantomData<T>);
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

use super::MaybeStaticSqlError;

trait FromKey {
    async fn get(id: Key<Self>) -> Result<Self, MaybeStaticSqlError> where Self: Sized;
}

#[derive(sqlx::FromRow)]
struct Session {
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
    async fn get(id: Key<Self>) -> Result<Self, MaybeStaticSqlError> {
        let pool = crate::store::DB_POOL.get().await?;
        let session = sqlx::query_as::<_, Self>(r#"SELECT * FROM session WHERE id = ?"#)
            .bind(id)
            .fetch_one(&pool)
            .await?;
        Ok(session)
    }
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
struct Error {
    id: Key<Self>,
    timestamp: chrono::DateTime<chrono::Utc>,
    fmt_display: String,
    fmt_debug: String,
    session: Key<Session>,
}


#[derive(sqlx::FromRow)]
struct PendingDispatch {
    id: Key<Self>,
    timestamp: chrono::DateTime<chrono::Utc>,
    backend: String,
    #[sqlx(rename = "track")] track: Key<DeferredTrack>,
    #[sqlx(rename = "error")] error: Key<Error>,
}
