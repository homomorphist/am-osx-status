use sqlx::{Encode, FromRow};
use sqlx::decode::Decode;
use sqlx::types::Type;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct BadTimestamp;
impl std::error::Error for BadTimestamp {}
impl std::fmt::Display for BadTimestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid timestamp value")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct MillisecondTimestamp(pub chrono::DateTime<chrono::Utc>);
impl Encode<'_, sqlx::Sqlite> for MillisecondTimestamp {
    fn encode_by_ref(
            &self,
            buf: &mut <sqlx::Sqlite as sqlx::Database>::ArgumentBuffer<'_>,
        ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        let millis = self.0.timestamp_millis();
        <i64 as Encode<sqlx::Sqlite>>::encode_by_ref(&millis, buf)
    }
}
impl Decode<'_, sqlx::Sqlite> for MillisecondTimestamp {
    fn decode(value: sqlx::sqlite::SqliteValueRef<'_>) -> Result<Self, sqlx::error::BoxDynError> {
        let millis: i64 = Decode::<sqlx::Sqlite>::decode(value)?;
        let dt = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(millis).ok_or(BadTimestamp)?;
        Ok(MillisecondTimestamp(dt))
    }
}
impl Type<sqlx::Sqlite> for MillisecondTimestamp {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <i64 as Type<sqlx::Sqlite>>::type_info()
    }
    fn compatible(ty: &<sqlx::Sqlite as sqlx::Database>::TypeInfo) -> bool {
        <i64 as Type<sqlx::Sqlite>>::compatible(ty)
    }
}
impl AsRef<chrono::DateTime<chrono::Utc>> for MillisecondTimestamp {
    fn as_ref(&self) -> &chrono::DateTime<chrono::Utc> {
        &self.0
    }
}
impl AsMut<chrono::DateTime<chrono::Utc>> for MillisecondTimestamp {
    fn as_mut(&mut self) -> &mut chrono::DateTime<chrono::Utc> {
        &mut self.0
    }
}
impl From<chrono::DateTime<chrono::Utc>> for MillisecondTimestamp {
    fn from(dt: chrono::DateTime<chrono::Utc>) -> Self {
        MillisecondTimestamp(dt)
    }
}
impl From<MillisecondTimestamp> for chrono::DateTime<chrono::Utc> {
    fn from(val: MillisecondTimestamp) -> Self {
        val.0
    }
}
impl PartialEq<chrono::DateTime<chrono::Utc>> for MillisecondTimestamp {
    fn eq(&self, other: &chrono::DateTime<chrono::Utc>) -> bool {
        self.0 == *other
    }
}
impl PartialOrd<chrono::DateTime<chrono::Utc>> for MillisecondTimestamp {
    fn partial_cmp(&self, other: &chrono::DateTime<chrono::Utc>) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(other)
    }
}
impl From<i64> for MillisecondTimestamp {
    fn from(millis: i64) -> Self {
        let dt = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(millis)
            .unwrap_or_else(|| panic!("timestamp millis out of valid date range: {millis}"));

        MillisecondTimestamp(dt)
    }
}

/// SQLite doesn't support 8-bit unsigned integers, so use an i64 as an intermediary representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct StoredPersistentId(i64);
impl StoredPersistentId {
    pub const fn new(id: u64) -> Self {
        Self(u64::cast_signed(id))
    }

    pub fn from_hex(value: &str) -> Result<Self, core::num::ParseIntError> {
        Ok(Self::new(u64::from_str_radix(value, 16)?))       
    }

    pub fn to_hex_upper(self) -> String {
        format!("{:X}", self.0)
    }
    pub fn to_hex_lower(self) -> String {
        format!("{:x}", self.0)
    }

    pub fn get(&self) -> u64 {
        i64::cast_unsigned(self.0)
    }

    pub fn signed(&self) -> i64 {
        self.0
    }
}
impl From<StoredPersistentId> for u64 {
    fn from(val: StoredPersistentId) -> Self {
        val.get()
    }
}
impl From<u64> for StoredPersistentId {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}
impl sqlx::Encode<'_, sqlx::Sqlite> for StoredPersistentId {
    fn encode_by_ref(
            &self,
            buf: &mut <sqlx::Sqlite as sqlx::Database>::ArgumentBuffer<'_>,
        ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        <i64 as sqlx::Encode<sqlx::Sqlite>>::encode_by_ref(&self.0, buf)
    }
}
impl sqlx::Decode<'_, sqlx::Sqlite> for StoredPersistentId {
    fn decode(value: sqlx::sqlite::SqliteValueRef<'_>) -> Result
        <Self, sqlx::error::BoxDynError> {
        let signed: i64 = sqlx::Decode::<sqlx::Sqlite>::decode(value)?;
        Ok(StoredPersistentId(signed))
    }
}
impl sqlx::Type<sqlx::Sqlite> for StoredPersistentId {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <i64 as sqlx::Type<sqlx::Sqlite>>::type_info()
    }
    fn compatible(ty: &<sqlx::Sqlite as sqlx::Database>::TypeInfo) -> bool {
        <i64 as sqlx::Type<sqlx::Sqlite>>::compatible(ty)
    }
}
impl core::fmt::Display for StoredPersistentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.get())
    }
}
#[cfg(feature = "musicdb")]
impl<T> From<StoredPersistentId> for musicdb::PersistentId<T> {
    fn from(val: StoredPersistentId) -> Self {
        musicdb::PersistentId::new(val.get())
    }
}
#[cfg(feature = "musicdb")]
impl<T> From<musicdb::PersistentId<T>> for StoredPersistentId {
    fn from(value: musicdb::PersistentId<T>) -> Self {
        Self::new(value.get_raw())
    }
}

#[cfg(test)]
mod tests {
    use mzstatic::pool;

    use super::*;
    use super::super::debug::*;

    #[tokio::test]
    async fn stored_persistent_id() {
        mk_test_db!("stored-persistent-id", pool);

        #[derive(sqlx::FromRow)]
        struct TestRow { value: StoredPersistentId }
  
        const VALUE: u64 = 10213095753550683260;

        assert!(VALUE > i64::MAX as u64, "test value must be greater than i64::MAX to test preservation in casting");

        sqlx::query("CREATE TABLE test (value INTEGER PRIMARY KEY NOT NULL);")
            .execute(&pool)
            .await
            .expect("failed to create table");

        sqlx::query("INSERT INTO test (value) VALUES (?);")
            .bind(StoredPersistentId::new(VALUE))
            .execute(&pool)
            .await
            .expect("failed to insert");

        let fetched: TestRow = sqlx::query_as("SELECT * FROM test").fetch_one(&pool).await.expect("failed to fetch");

        assert_eq!(fetched.value.get(), VALUE, "value is preserved across boundaries");
    }
}

    