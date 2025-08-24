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
            .expect("timestamp millis out of valid date range");
        MillisecondTimestamp(dt)
    }
}
