#![expect(unused, reason = "used in tests or debugging")]

pub mod dump;

#[macro_export]
macro_rules! mk_test_db {
    ($name: literal, $ident: ident, seed: true) => {
        mk_test_db!($name, $ident);
        super::seed_empty(&pool).await;
    };
    ($name: literal, $ident: ident) => {
        static POOL: $crate::store::GlobalPool = $crate::store::GlobalPool::new(|| {
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

        let $ident = POOL.get().await.expect("failed to get pool");
    }
}

pub use mk_test_db;

pub const SEED_SQL: &str = include_str!("../sql/seeding/initial.sql");

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

#[macro_export]
macro_rules! assert_eq_diff {
    (@@ $left:expr, $right:expr, $msg_delim: literal, $msg:expr) => {
        {
            #[cfg(unix)]
            {
                let left = $left;
                let right = $right;
                if $left != right {
                    const ANSI_RESET: &str = "\x1b[0m";

                    let script = r#"diff -u --color=always <(printf '%s' "$1") <(printf '%s' "$2")"#;
                    let output = std::process::Command::new("bash")
                        .arg("-c").arg(script)
                        .arg("--").arg(left).arg(right)
                        .output();

                    let diff = match output {
                        Ok(output) => String::from_utf8_lossy(&output.stdout).trim().to_string(),
                        Err(e) => panic!("Failed to run diff command: {e}")
                    };

                    panic!("assertion `left == right` failed{}{}\n=== BEGIN DIFF ===\n{diff}{ANSI_RESET}\n=== END DIFF ===\n", $msg_delim, $msg);
                }
            }
            #[cfg(not(unix))]
            {
                assert_eq!($left, $right, "assertion failed: `(left == right)`\n\n(Diff output is only available on Unix-like systems.)");
            }
        }
    };
    ($left:expr, $right:expr $(,)?) => {
        assert_eq_diff!(@@ $left, $right, "", "");
    };
    ($left:expr, $right:expr, $($arg:tt)+) => {
        assert_eq_diff!(@@ $left, $right, ": ", format!($($arg)+));
    };
}

pub use assert_eq_diff;
