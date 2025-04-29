use super::{GlobalPool, DB_POOL};
use include_dir::Dir;
use sqlx::{Row, Column};

type Epoch = chrono::DateTime<chrono::Utc>;

static MIGRATIONS_DIRECTORY: Dir<'static> = include_dir::include_dir!("$CARGO_MANIFEST_DIR/src/store/sql/migrations/");

#[derive(Debug)]
struct Migration {
    name: &'static str,
    epoch: u32,
    sql_up: &'static str,
    sql_down: &'static str,
}
impl From<&include_dir::Dir<'static>> for Migration {
    fn from(folder: &include_dir::Dir<'static>) -> Self {
        let folder_name = folder.path().file_name()
            .expect("migration has no name").to_str()
            .expect("migration name is not valid utf8");

        let (epoch, name) = folder_name.split_once('-').expect("no epoch delimiter present on migration file");
        let epoch = u32::from_str_radix(epoch, 16).expect("epoch is not a valid hex number");

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

fn get_migrations() -> Vec<Migration> {
    let mut migrations = MIGRATIONS_DIRECTORY.dirs()
        .map(Migration::from)
        .collect::<Vec<_>>();

    migrations.sort_by_key(|migration| migration.epoch);
    migrations
}

fn is_from_missing_session_table(err: &sqlx::Error) -> bool {
    err.as_database_error().is_some_and(|v| v.message() == "no such table: session")
}

#[tokio::test]
async fn migrate() {
    let migrations = get_migrations();
    let pool = DB_POOL.get().await.expect("failed to get pool");

    let last = get_last_run_epoch().await;
    let mut last = last.unwrap_or(0);

    for migration in migrations {
        if migration.epoch > last {
            sqlx::query(migration.sql_up)
                .execute(&pool)
                .await
                .expect("failed to run migration");
            last = migration.epoch;
        }
    }
}

async fn get_last_run_epoch() -> Option<u32> {
    sqlx::query("SELECT started_at FROM session ORDER BY started_at DESC LIMIT 1")
        .fetch_optional(&DB_POOL.get().await.expect("failed to get pool"))
        .await
        .or_else(|err| { if is_from_missing_session_table(&err) { Ok(None) } else { Err(err) } })
        .expect("failed to get last run epoch")
        .map(|v| v.get::<u32, _>(0))
}

mod tests {
    use super::*;

    mod util {
        use super::*;

        #[macro_export]
        macro_rules! mk_test_db {
            ($name: literal, $ident: ident, seed: true) => {
                mk_test_db!($name, $ident);
                super::seed_empty(&pool).await;
            };
            ($name: literal, $ident: ident) => {
                static POOL: GlobalPool = GlobalPool::new(|| {
                    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
                    let connect = SqliteConnectOptions::new()
                        .filename($name)
                        .in_memory(true)
                        .create_if_missing(true);
                    let pool = SqlitePoolOptions::new().max_connections(1);
                    super::super::GlobalPoolOptions { connect, pool }
                });

                let $ident = POOL.get().await.expect("failed to get pool");
            }
        }

        pub use mk_test_db;

        pub async fn dump_schema_into(string: &mut String, pool: &sqlx::SqlitePool) {
            let schema = sqlx::query("SELECT sql FROM sqlite_master WHERE type IN ('table', 'index', 'trigger', 'view')")
                .fetch_all(pool)
                .await
                .expect("failed to get schema");
            for row in schema {
                string.push_str(row.get::<String, _>(0).as_str());
                string.push('\n');
            };
        }
        pub async fn dump_schema(pool: &sqlx::SqlitePool) -> String {
            let mut string = String::new();
            dump_schema_into(&mut string, pool).await;
            string
        }

        pub async fn dump_table_into(string: &mut String, pool: &sqlx::SqlitePool, table: &str) {
            let output = sqlx::query(&format!("SELECT * FROM {table}"))
                .fetch_all(pool)
                .await
                .expect("failed to execute query");
            string.push('\n');
            string.push_str(&format!("{table}:"));
            for row in output {
                string.push('\n');
                string.push('(');
                for column in row.columns() {
                    use sqlx::sqlite::SqliteValueRef;
                    let value = match column.type_info().to_string().as_str() {
                        "TEXT" => row.get::<String, _>(column.name()),
                        "INTEGER" => row.get::<i64, _>(column.name()).to_string(),
                        "REAL" => row.get::<f64, _>(column.name()).to_string(),
                        "BLOB" => unimplemented!(),
                        "NULL" => "NULL".to_string(),
                        _ => unimplemented!(),
                    };
                    string.push_str(&value);
                    string.push(',');
                }
                string.push(')');
            };
        }
        pub async fn dump_table(pool: &sqlx::SqlitePool, table: &str) -> String {
            let mut string = String::new();
            dump_table_into(&mut string, pool, table).await;
            string
        }

        pub async fn dump_tables_into<T: AsRef<str>>(string: &mut String, pool: &sqlx::SqlitePool, tables: impl Iterator<Item = T>) {
            for table in tables {
                dump_table_into(string, pool, table.as_ref()).await;
            }
        }
        pub async fn dump_tables<T: AsRef<str>>(pool: &sqlx::SqlitePool, tables: impl Iterator<Item = T>) -> String {
            let mut string = String::new();
            dump_tables_into(&mut string, pool, tables).await;
            string
        }

        pub async fn get_tables(pool: &sqlx::SqlitePool) -> impl Iterator<Item = String> {
            let tables = sqlx::query("SELECT name FROM sqlite_master WHERE type='table'")
                .fetch_all(pool)
                .await
                .expect("failed to get tables");
            tables.into_iter()
                .map(|row| row.get::<String, _>(0))
                .filter(|table| table != "sqlite_sequence")
        }


        pub async fn dump_database(pool: &sqlx::SqlitePool) -> String {
            let mut string = String::new();
            let tables: Vec<_> = get_tables(pool).await.collect();
            dump_schema_into(&mut string, pool).await;
            dump_tables_into(&mut string, pool, tables.iter()).await;
            string
        }

        const SEED_SQL: &str = include_str!("./sql/seeding/initial.sql");

        pub async fn seed_empty(pool: &sqlx::SqlitePool, migrate: bool) {
            // apply first migration to initialize schema
            let migrations = get_migrations();
            sqlx::query(migrations[0].sql_up)
                .execute(pool)
                .await
                .expect("failed to run migration");

            sqlx::query(SEED_SQL)
                .execute(pool)
                .await
                .expect("failed to seed");

            if migrate {
                // apply remaining
                for migration in &migrations[1..] {
                    sqlx::query(migration.sql_up)
                        .execute(pool)
                        .await
                        .expect("failed to run migration");
                }
            }
        }
    }

    use util::*;

    #[tokio::test]
    async fn seeding() {
        mk_test_db!("seeded", pool);

        const SQL: &str = include_str!("./sql/seeding/initial.sql");
        
        assert!(dump_database(&pool).await.is_empty());

        let tables: Vec<_> = get_tables(&pool).await.collect();
        assert!(!dump_schema(&pool).await.is_empty(), "schema is added");
        assert_eq!(dump_tables(&pool, tables.iter()).await.matches('\n').count(), tables.len(), "no data is added");

        sqlx::query(SQL)
            .execute(&pool)
            .await
            .expect("failed to seed");

        assert!(dump_tables(&pool, tables.iter()).await.matches('\n').count() > tables.len(), "data is added");
    }

    #[tokio::test]
    async fn full_up_and_down_schema_migration() {
        mk_test_db!("migrated-schema", pool);
        let last = get_last_run_epoch().await;
        let migrations = get_migrations();

        let mut schema_dumps = vec![];
        for migration in &migrations {
            schema_dumps.push(dump_database(&pool).await);
            sqlx::query(migration.sql_up)
                .execute(&pool)
                .await
                .expect("failed to run migration");
        }

        for migration in migrations.into_iter().rev() {
            sqlx::query(migration.sql_down)
                .execute(&pool)
                .await
                .expect("failed to run migration");
            assert_eq!(
                dump_database(&pool).await,
                schema_dumps.pop().expect("schema dump is missing"),
                "downwards migration does not result in the same schema dump"
            );
        }
    }
}