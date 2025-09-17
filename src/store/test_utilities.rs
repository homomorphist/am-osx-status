use sqlx::{Row, Column};

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
                .create_if_missing(true)
                .statement_cache_capacity(0);
                // no cache to avoid cached query fuck-ups from altering a table
                // this is also a problem in non-test environments but we can refresh the pool for that
                // that isn't an option here though since it's in-memory and would reset so we work-around
            let pool = SqlitePoolOptions::new().max_connections(1);
            super::super::GlobalPoolOptions { connect, pool }
        });

        let mut $ident = POOL.get().await.expect("failed to get pool");
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

pub const SEED_SQL: &str = include_str!("./sql/seeding/initial.sql");

pub async fn apply_migrations(pool: &sqlx::SqlitePool, migrations: &[super::migrations::Migration]) {
    for migration in migrations {
        sqlx::query(migration.sql_up)
            .execute(pool)
            .await
            .expect("failed to run migration");
    }
}

pub async fn seed_empty(pool: &sqlx::SqlitePool, migrate: bool) {
    // apply first migration to initialize schema
    let migrations = super::migrations::get_migrations();
    sqlx::query(migrations[0].sql_up)
        .execute(pool)
        .await
        .expect("failed to run migration");

    sqlx::query(SEED_SQL)
        .execute(pool)
        .await
        .expect("failed to seed");

    if migrate {
        apply_migrations(pool, &migrations[1..]).await;
    }
}
