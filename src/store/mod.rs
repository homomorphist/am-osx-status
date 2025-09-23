use std::sync::LazyLock;
use tokio::sync::Mutex;

pub mod migrations;
pub mod types;
pub mod entities;

#[cfg(any(test, debug_assertions))]
pub(crate) mod debug;

pub static DB_PATH: LazyLock<std::path::PathBuf> = LazyLock::new(|| {
    crate::util::APPLICATION_SUPPORT_FOLDER.join("sqlite.db")
});

pub static DB_POOL: GlobalPool = GlobalPool::new(|| {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    let connect = SqliteConnectOptions::new()
        .filename(DB_PATH.as_path())
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new().max_connections(3);
    GlobalPoolOptions { connect, pool }
});


pub struct GlobalPoolOptions {
    pub connect: sqlx::sqlite::SqliteConnectOptions,
    pub pool: sqlx::sqlite::SqlitePoolOptions,
}
pub struct GlobalPool {
    options: fn() -> GlobalPoolOptions,
    inner: Mutex<Option<sqlx::SqlitePool>>,
    error: Mutex<Option<&'static sqlx::Error>>,
}
impl GlobalPool {
    pub const fn new(options: fn() -> GlobalPoolOptions) -> Self {
        Self {
            options,
            inner: Mutex::const_new(None),
            error: Mutex::const_new(None),
        }
    }

    pub async fn get(&self) -> Result<sqlx::SqlitePool, &'static sqlx::Error> {
        {
            if let Some(pool) = &*self.inner.lock().await {
                return Ok(pool.clone())
            } else if let Some(error) = &*self.error.lock().await {
                return Err(error)
            }
        }
        
        let options = (self.options)();
        match options.pool.connect_with(options.connect).await {
            Ok(pool) => {
                *self.inner.lock().await = Some(pool.clone());
                Ok(pool)
            }
            Err(e) => {
                let error = &*Box::leak(Box::new(e));
                *self.error.lock().await = Some(error);
                Err(error)
            }
        }
    }

    /// Totally refresh the pool.
    /// This will either panic or cause multiple pools to exist if called at an inopportune time. Maybe. I dunno.
    pub async fn refresh(&self) {
        let mut db = self.inner.try_lock().expect("cannot obtain lock");
        if let Some(db) = db.take() { db.close(); }
        *self.error.try_lock().expect("cannot obtain lock") = None;
    }
}

/// A wrapper around `sqlx::Error` that can be either a reference to a static error or a dynamic error.
/// A static error would occur when a global pool failed to correctly initialize.
#[derive(Debug, thiserror::Error)]
pub enum MaybeStaticSqlError {
    #[error(transparent)]
    Owned(#[from] sqlx::Error),
    #[error(transparent)]
    Static(#[from] &'static sqlx::Error)
}
impl core::ops::Deref for MaybeStaticSqlError {
    type Target = sqlx::Error;
    fn deref(&self) -> &Self::Target {
        match self {
            MaybeStaticSqlError::Owned(e) => e,
            MaybeStaticSqlError::Static(e) => e
        }
    }
}
