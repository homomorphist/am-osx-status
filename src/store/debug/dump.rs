use chrono::format;
use core::fmt::Write;
use sqlx::{Row, Column};

pub async fn dump_schema_into(out: &mut String, pool: &sqlx::SqlitePool) {
    for table in get_tables(pool).await {
        out.push_str("TABLE ");
        out.push_str(&table);
        out.push('\n');
    
        out.push_str("  COLUMNS:\n");
        for col in get_column_info(pool, &table).await {
            out.push_str("    ");
            out.push_str(&col.name);
            out.push(' ');
            out.push_str(&col.datatype);
            if col.primary_key { out.push_str(" PRIMARY KEY"); }
            if col.not_null { out.push_str(" NOT NULL"); }
            if let Some(def) = col.default {
                out.push_str(" DEFAULT ");
                out.push_str(&def);
            }
            out.push_str(" -- ");
            out.push_str(col.visibility.to_str());
            out.push('\n');
        }

        out.push_str("  FOREIGN KEYS:\n");
        for fk in get_foreign_keys(pool, &table).await {
            out.push_str("    ");
            out.push_str(&fk.from);
            out.push_str(" -> ");
            out.push_str(&fk.table);
            out.push('(');
            out.push_str(&fk.to);
            out.push(')');
            if fk.on_update != ForeignKeyAction::NoAction {
                out.push_str(" ON UPDATE ");
                out.push_str(fk.on_update.as_ref());
            }
            if fk.on_delete != ForeignKeyAction::NoAction {
                out.push_str(" ON DELETE ");
                out.push_str(fk.on_delete.as_ref());
            }
            out.push('\n');
        }

        out.push('\n');
    }
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
    write!(string, "{table}:");
    for row in output {
        string.push_str("\n  (");
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

pub async fn get_tables(pool: &sqlx::SqlitePool) -> Vec<String> {
    sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .fetch_all(pool)
        .await
        .expect("failed to get tables")
        .into_iter()
        .map(|row| row.get::<String, _>(0))
        .collect()
}


#[derive(sqlx::Type, Debug, PartialEq, Eq)]
#[repr(u8)]
enum ColumnVisibility {
    Normal = 0,
    HiddenVirtual = 1,
    GeneratedDynamic = 2,
    GeneratedStored = 3,
}
impl ColumnVisibility {
    const fn to_str(&self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::HiddenVirtual => "HIDDEN VIRTUAL",
            Self::GeneratedDynamic => "GENERATED DYNAMIC",
            Self::GeneratedStored => "GENERATED STORED",
        }
    }
}

#[derive(sqlx::FromRow)]
struct TableColumnInfo {
    /// The name of the table.
    name: String,
    /// The type of the column.
    #[sqlx(rename = "type")]
    datatype: String,
    /// Whether the column isn't nullable.
    #[sqlx(rename = "notnull")]
    not_null: bool,
    /// The default value of the column.
    #[sqlx(rename = "dflt_value")]
    default: Option<String>,
    /// Whether this column is the primary key for the table.
    #[sqlx(rename = "pk")]
    primary_key: bool,
    /// The visibility of the column.
    #[sqlx(rename = "hidden")]
    visibility: ColumnVisibility
}

async fn get_column_info(pool: &sqlx::SqlitePool, table: &str) -> Vec<TableColumnInfo> {
    sqlx::query("SELECT * FROM pragma_table_xinfo(?)")
        .bind(table)
        .fetch_all(pool)
        .await
        .expect("failed to get columns")
        .into_iter()
        .map(|row| sqlx::FromRow::from_row(&row).expect("failed to parse column info"))
        .collect()
}


#[derive(sqlx::FromRow)]
struct TableForeignKeyInfo {
    id: i64,
    seq: i64,
    /// The table being referenced.
    table: String,
    /// The column in the current table.
    from: String,
    /// The column in the referenced table.
    to: String,
    on_update: ForeignKeyAction,
    on_delete: ForeignKeyAction,
    r#match: String, // ?
}

/// - <https://sqlite.org/foreignkeys.html#fk_actions>
#[derive(strum::EnumString, strum::AsRefStr, PartialEq, Eq)]
enum ForeignKeyAction {
    #[strum(serialize = "NO ACTION")]
    NoAction,
    #[strum(serialize = "RESTRICT")]
    Restrict,
    #[strum(serialize = "SET NULL")]
    SetNull,
    #[strum(serialize = "SET DEFAULT")]
    SetDefault,
    #[strum(serialize = "CASCADE")]
    Cascade,
}
impl sqlx::Type<sqlx::Sqlite> for ForeignKeyAction {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <String as sqlx::Type<sqlx::Sqlite>>::type_info()
    }
    fn compatible(ty: &sqlx::sqlite::SqliteTypeInfo) -> bool {
        <String as sqlx::Type<sqlx::Sqlite>>::compatible(ty)
    }
}
impl sqlx::Decode<'_, sqlx::Sqlite> for ForeignKeyAction {
    fn decode(value: sqlx::sqlite::SqliteValueRef<'_>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <String as sqlx::Decode<'_, sqlx::Sqlite>>::decode(value)?;
        s.parse().map_err(|_| "invalid foreign key action".into())
    }
}

async fn get_foreign_keys(pool: &sqlx::SqlitePool, table: &str) -> Vec<TableForeignKeyInfo> {
    sqlx::query("SELECT * FROM pragma_foreign_key_list(?)")
        .bind(table)
        .fetch_all(pool)
        .await
        .expect("failed to get foreign keys")
        .into_iter()
        .map(|row| sqlx::FromRow::from_row(&row).expect("failed to parse foreign key info"))
        .collect()
}

pub async fn dump_database(pool: &sqlx::SqlitePool) -> String {
    let mut string = String::new();
    let tables = get_tables(pool).await;
    dump_schema_into(&mut string, pool).await;
    dump_tables_into(&mut string, pool, tables.iter()).await;
    string
}
