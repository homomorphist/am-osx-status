/// A contextless hyphenated UUID string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct HyphenatedUuidString([u8; HyphenatedUuidString::BYTE_LENGTH]);
impl HyphenatedUuidString {
    const BYTE_LENGTH: usize = 36;

    pub const fn new(slice: &str) -> Option<HyphenatedUuidString> {
        if slice.len() == HyphenatedUuidString::BYTE_LENGTH && uuid::Uuid::try_parse(slice).is_ok() {
            Some(HyphenatedUuidString(*unsafe {
                *core::mem::transmute::<
                    &&[u8],
                    &&[u8; HyphenatedUuidString::BYTE_LENGTH]
                >(&slice.as_bytes())
            }))
        } else {
            None
        }
    }

    /// # Safety
    /// - The slice must be a valid hyphenated UUID, taking up 36 bytes.
    pub const unsafe fn new_unchecked(slice: &str) -> HyphenatedUuidString {
        HyphenatedUuidString(*unsafe {
            *core::mem::transmute::<
                &&[u8],
                &&[u8; HyphenatedUuidString::BYTE_LENGTH]
            >(&slice.as_bytes())
        })
    }

    pub const fn as_str(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(&self.0) }
    }
}
impl AsRef<str> for HyphenatedUuidString {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
impl core::fmt::Display for HyphenatedUuidString{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}
impl serde::Serialize for HyphenatedUuidString {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_ref())
    }
}
impl<'de> serde::Deserialize<'de> for HyphenatedUuidString {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = HyphenatedUuidString;

            fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
                formatter.write_str("a hyphenated UUID-like string")
            }

            fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E> where E: Error {
                HyphenatedUuidString::new(value).ok_or_else(|| serde::de::Error::custom("invalid UUID"))
            }
            fn visit_string<E>(self, value: String) -> Result<Self::Value, E> where E: Error {
                HyphenatedUuidString::new(&value).ok_or_else(|| serde::de::Error::custom("invalid UUID"))
            }
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> where E: Error {
                HyphenatedUuidString::new(value).ok_or_else(|| serde::de::Error::custom("invalid UUID"))
            }
        }


        deserializer.deserialize_str(Visitor)
    }
}
