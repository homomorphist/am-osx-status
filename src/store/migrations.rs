use super::{GlobalPool, DB_POOL};
use include_dir::Dir;
use sqlx::{Row, Column};

type Epoch = chrono::DateTime<chrono::Utc>;

static MIGRATIONS_DIRECTORY: Dir<'static> = include_dir::include_dir!("$CARGO_MANIFEST_DIR/src/store/sql/migrations/");

#[derive(Debug)]
pub struct Migration {
    pub name: &'static str,
    pub epoch: chrono::DateTime<chrono::Utc>,
    pub sql_up: &'static str,
    pub sql_down: &'static str,
}
impl From<&include_dir::Dir<'static>> for Migration {
    fn from(folder: &include_dir::Dir<'static>) -> Self {
        let folder_name = folder.path().file_name()
            .expect("migration has no name").to_str()
            .expect("migration name is not valid utf8");

        let (epoch, name) = folder_name.split_once('-').expect("no epoch delimiter present on migration file");
        let epoch = u32::from_str_radix(epoch, 16).expect("epoch is not a valid hex number");
        let epoch = Epoch::from_timestamp(epoch as i64, 0).expect("epoch out of valid date range");

        macro_rules! get {
            ($name:literal) => {
                folder.get_file({
                    let mut path = folder.path().to_owned();
                    path.push($name);
                    path
                }).expect(concat!("no ", $name, " file present on migration folder")).contents_utf8().expect(concat!($name, " is not valid utf8"))
            };
        }

        let sql_up = get!("up.sql");
        let sql_down = get!("down.sql");

        Self {
            name,
            epoch,
            sql_up,
            sql_down,
        }
    }
}

pub(super) fn get_migrations() -> Vec<Migration> {
    let mut migrations = MIGRATIONS_DIRECTORY.dirs()
        .map(Migration::from)
        .collect::<Vec<_>>();

    migrations.sort_by_key(|migration| migration.epoch);
    migrations
}

fn is_from_missing_sessions_table(err: &sqlx::Error) -> bool {
    err.as_database_error().is_some_and(|v| v.message() == "no such table: sessions")
}

async fn migrate() {
    let migrations = get_migrations();
    let pool = DB_POOL.get().await.expect("failed to get pool");
    let last = get_last_run_epoch().await;

    for migration in migrations {
        if last.is_none_or(|v| v < migration.epoch) {
            sqlx::query(migration.sql_up)
                .execute(&pool)
                .await
                .expect("failed to run migration");
        }
    }
}

async fn get_last_run_epoch() -> Option<Epoch> {
    sqlx::query("SELECT started_at FROM sessions ORDER BY started_at DESC LIMIT 1")
        .fetch_optional(&DB_POOL.get().await.expect("failed to get pool"))
        .await
        .or_else(|err| { if is_from_missing_sessions_table(&err) { Ok(None) } else { Err(err) } })
        .expect("failed to get last run epoch")
        .map(|v| v.get::<u32, _>(0))
        .and_then(|v| match Epoch::from_timestamp(v as i64, 0) {
            Some(v) => Some(v),
            None => { tracing::error!(?v, "sessions epoch out of valid date range; ignoring"); None }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::test_utilities::*;

    #[tokio::test]
    async fn seeding() {
        mk_test_db!("seeded", pool);
        
        assert!(dump_database(&pool).await.is_empty());
        
        let init = get_migrations().into_iter().next().expect("no migrations found");
        sqlx::query(init.sql_up)
            .execute(&pool)
            .await
            .expect("failed to run migration");

        let tables: Vec<_> = get_tables(&pool).await.collect();

        assert!(!dump_schema(&pool).await.is_empty(), "schema is added");
        assert_eq!(dump_tables(&pool, tables.iter()).await.matches('\n').count(), tables.len(), "no data is added");

        sqlx::query(SEED_SQL)
            .execute(&pool)
            .await
            .expect("failed to seed");

        assert!(dump_tables(&pool, tables.iter()).await.matches('\n').count() > tables.len(), "data is added");
    }

    #[tokio::test]
    async fn full_up_and_down_migration() {
        mk_test_db!("up-and-down", pool);
        let mut dumps = vec![dump_database(&pool).await];
        let migrations = get_migrations();
        let (init, migrations) = migrations.split_first().expect("no migrations found");
        
        sqlx::query(init.sql_up)
            .execute(&pool)
            .await
            .expect("failed to initialize schema");
        
        sqlx::query(SEED_SQL)
            .execute(&pool)
            .await
            .expect("failed to seed");

        for migration in migrations {
            sqlx::query(migration.sql_up)
                .execute(&pool)
                .await
                .expect("failed to run migration");
            dumps.push(dump_database(&pool).await);
        }

        for migration in migrations.iter().rev() {
            sqlx::query(migration.sql_down)
                .execute(&pool)
                .await
                .expect("failed to run migration");
            assert_eq!(
                dump_database(&pool).await,
                dumps.pop().expect("no more dumps"),
                "downwards migration results in identity database state"
            );
        }

        sqlx::query(init.sql_down)
            .execute(&pool)
            .await
            .expect("failed to run migration");

        assert_eq!(
            dump_database(&pool).await,
            dumps.pop().expect("no more dumps"),
            "final downwards migration results in an empty database"
        );

        assert!(dumps.is_empty(), "undid all migrations");
    }
}
