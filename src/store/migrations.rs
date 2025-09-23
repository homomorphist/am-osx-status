use crate::store::types::MillisecondTimestamp;

use super::{GlobalPool, DB_POOL};
use include_dir::Dir;
use sqlx::{Row, Column};

static MIGRATIONS_DIRECTORY: Dir<'static> = include_dir::include_dir!("$CARGO_MANIFEST_DIR/src/store/sql/migrations/");

/// Zero signifies no migrations, including any initialization, such that the database is empty.
pub type MigrationID = u16;
type MigrationQuantity = MigrationID;

#[derive(Debug)]
pub struct Migration {
    pub name: &'static str,
    pub id: MigrationID,
    pub sql_up: &'static str,
    pub sql_down: &'static str,
}
impl From<&include_dir::Dir<'static>> for Migration {
    fn from(folder: &include_dir::Dir<'static>) -> Self {
        let folder_name = folder.path().file_name()
            .expect("migration has no name").to_str()
            .expect("migration name is not valid utf8");

        let (id, name) = folder_name.split_once(':').expect("no id delimiter present on migration folder");
        let id = id.parse::<MigrationID>().expect("migration id is not a number");
        assert_ne!(id, 0, "no migration can exist with an id of zero");

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
            id,
            sql_up,
            sql_down,
        }
    }
}

pub(super) fn get_migrations() -> Vec<Migration> {
    let mut migrations = MIGRATIONS_DIRECTORY.dirs()
        .map(Migration::from)
        .collect::<Vec<_>>();

    migrations.sort_by_key(|migration| migration.id);
    
    for (i, migration) in migrations.iter().enumerate() {
        assert_eq!(migrations[i].id as usize, i + 1, "migration ids must be sequential starting from 1");
    }

    migrations
}

fn is_from_missing_sessions_table(err: &sqlx::Error) -> bool {
    err.as_database_error().is_some_and(|v| v.message() == "no such table: sessions")
}

/// Returns the new migration ID.
/// 
/// This will refresh the pool on every single call to prevent cached queries from being fucked
/// up by table alterations. As such, the DB shouldn't be in use while this is being called,
/// or they might be caught in the crossfire and multiple pools could exist.
/// (Though it'd probably be a bad idea to use the DB while this is happening in the first place...)
pub async fn migrate() -> MigrationID {
    let mut id = get_last_migration_id().await;
    let mut pool = DB_POOL.get().await.expect("failed to get pool");
    let migrations = get_migrations();

    for migration in migrations.iter().filter(|m| m.id > id) {
        tracing::debug!(?migration, "applying migration");
        sqlx::query(migration.sql_up)
            .execute(&pool)
            .await
            .expect("failed to run migration");

        DB_POOL.refresh().await;
        pool = DB_POOL.get().await.expect("failed to get pool");
    }

    migrations.last().map(|m| m.id).unwrap_or(0)
}

async fn get_last_migration_id() -> MigrationID {
    sqlx::query("SELECT migration_id FROM sessions ORDER BY started_at DESC LIMIT 1")
        .fetch_optional(&DB_POOL.get().await.expect("failed to get pool"))
        .await
        .ok()
        .flatten()
        .map(|row| row.get::<MigrationID, _>(0) as MigrationID)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::debug::*;
    use super::super::debug::dump::*;

    #[tokio::test]
    async fn seeding() {
        mk_test_db!("seeded", pool);
        
        assert!(dump_database(&pool).await.is_empty());
        
        let init = get_migrations().into_iter().next().expect("no migrations found");
        sqlx::query(init.sql_up)
            .execute(&pool)
            .await
            .expect("failed to run migration");

        let tables: Vec<_> = get_tables(&pool).await;

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
            let dump = dump_database(&pool).await;
            assert!(dumps.last() != Some(&dump), "each migration should result in a different database dump");
            dumps.push(dump);
            println!("applying migration {}: {}", migration.id, migration.name);
            sqlx::query(migration.sql_up)
                .execute(&pool)
                .await
                .expect("failed to run migration");
        }

        assert!(&dump_database(&pool).await != dumps.last().expect("no dumps"), "last migration changed the database");

        for migration in migrations.iter().rev() {
            sqlx::query(migration.sql_down)
                .execute(&pool)
                .await
                .expect("failed to run migration");
            assert_eq_diff!(
                &dump_database(&pool).await,
                &dumps.pop().expect("no more dumps"),
                "downwards migration results in identical database state"
            );
        }

        sqlx::query(init.sql_down)
            .execute(&pool)
            .await
            .expect("failed to run migration");

        assert_eq_diff!(
            dump_database(&pool).await,
            dumps.pop().expect("no more dumps"),
            "final downwards migration results in an empty database"
        );

        assert!(dumps.is_empty(), "undid all migrations");
    }
}
