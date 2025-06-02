#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
use alloc::string::{String, ToString};

/// A value that is either a reference to a string slice or an owned string.
#[derive(Debug)]
pub enum MaybeOwnedString<'a> {
    /// A borrowed string.
    Borrowed(&'a str),
    /// An owned string.
    Owned(String)
}
impl<'a> From<&'a MaybeOwnedString<'a>> for &'a str {
    fn from(value: &'a MaybeOwnedString<'a>) -> Self {
        match value {
            MaybeOwnedString::Borrowed(borrowed) => borrowed,
            MaybeOwnedString::Owned(owned) => owned
        }
    }
}
impl<'a> From<&MaybeOwnedString<'a>> for String {
    fn from(value: &MaybeOwnedString<'a>) -> Self {
        match value {
            MaybeOwnedString::Borrowed(borrowed) => borrowed.to_string(),
            MaybeOwnedString::Owned(owned) => owned.clone()
        }
    }
}
impl<'a> From<&'a str> for MaybeOwnedString<'a> {
    fn from(value: &'a str) -> Self {
        Self::Borrowed(value)
    }
}
impl From<String> for MaybeOwnedString<'_> {
    fn from(value: String) -> Self {
        Self::Owned(value)
    }
}
impl<'a> From<&'a String> for MaybeOwnedString<'a> {
    fn from(value: &'a String) -> Self {
        Self::Borrowed(value.as_str())
    }
}
impl AsRef<str> for MaybeOwnedString<'_> {
    fn as_ref(&self) -> &str {
        match self {
            Self::Borrowed(borrowed) => borrowed,
            Self::Owned(owned) => owned
        }
    }
}
impl core::ops::Deref for MaybeOwnedString<'_> {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Borrowed(borrowed) => borrowed,
            Self::Owned(owned) => owned
        }
    }
}
impl core::fmt::Display for MaybeOwnedString<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}
impl Default for MaybeOwnedString<'_> {
    fn default() -> Self {
        Self::Borrowed("")
    }
}
impl Clone for MaybeOwnedString<'_> {
    fn clone(&self) -> Self {
        match self {
            Self::Borrowed(borrowed) => Self::Borrowed(borrowed),
            Self::Owned(owned) => Self::Owned(owned.clone())
        }
    }
    fn clone_from(&mut self, source: &Self) {
        match self {
            Self::Borrowed(borrowed) => match source {
                Self::Borrowed(rhs) => *borrowed = rhs,
                Self::Owned(rhs) => *self = Self::Owned(rhs.to_string())
            },
            Self::Owned(owned) => match source {
                Self::Borrowed(rhs) => *owned = rhs.to_string(),
                Self::Owned(rhs) => alloc::borrow::ToOwned::clone_into(rhs, owned)
            }
        }
    }
}
impl PartialOrd for MaybeOwnedString<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for MaybeOwnedString<'_> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        let lhs = AsRef::<str>::as_ref(self);
        let rhs = AsRef::<str>::as_ref(other);
        lhs.cmp(rhs)
    }
}
impl PartialEq for MaybeOwnedString<'_> {
    fn eq(&self, other: &Self) -> bool {
        let lhs = AsRef::<str>::as_ref(self);
        let rhs = AsRef::<str>::as_ref(other);
        lhs.eq(rhs)
    }
}
impl PartialEq<str> for MaybeOwnedString<'_> {
    fn eq(&self, other: &str) -> bool {
        let lhs = AsRef::<str>::as_ref(self);
        lhs.eq(other)
    }
}
impl PartialEq<&str> for MaybeOwnedString<'_> {
    fn eq(&self, other: &&str) -> bool {
        let lhs = AsRef::<str>::as_ref(self);
        lhs.eq(*other)
    }
}
impl PartialEq<String> for MaybeOwnedString<'_> {
    fn eq(&self, other: &String) -> bool {
        let lhs = AsRef::<str>::as_ref(self);
        lhs.eq(other.as_str())
    }
}
impl PartialEq<MaybeOwnedString<'_>> for dyn AsRef<str> {
    fn eq(&self, other: &MaybeOwnedString<'_>) -> bool {
        let rhs = AsRef::<str>::as_ref(other);
        let lhs = self.as_ref();
        lhs.eq(rhs)
    }
}
impl Eq for MaybeOwnedString<'_> {}
impl core::hash::Hash for MaybeOwnedString<'_> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        state.write(match self {
            Self::Borrowed(borrowed) => borrowed.as_bytes(),
            Self::Owned(owned) => owned.as_bytes()
        })
    }
}
impl MaybeOwnedString<'_> {
    /// Whether this is a string slice reference.
    pub const fn is_borrowed(&self) -> bool {
        matches!(self, Self::Borrowed(_))
    }

    /// Whether an owned string is held.
    pub const fn is_owned(&self) -> bool {
        matches!(self, Self::Owned(_))
    }

    /// Returns the held inner owned string, if this string is owned.
    pub fn into_inner_owned(self) -> Option<String> {
        if let Self::Owned(owned) = self {
            Some(owned)
        } else {
            None
        }
    }
}

#[cfg(feature = "std")] use std::borrow::Cow;
#[cfg(feature = "std")] impl<'a> From<Cow<'a, str>> for MaybeOwnedString<'a> {
    fn from(value: Cow<'a, str>) -> Self {
        match value {
            Cow::Borrowed(borrowed) => Self::Borrowed(borrowed),
            Cow::Owned(owned) => Self::Owned(owned),
        }
    }
}
#[cfg(feature = "std")] impl<'a> From<MaybeOwnedString<'a>> for Cow<'a, str> {
    fn from(value: MaybeOwnedString<'a>) -> Self {
        match value {
            MaybeOwnedString::Borrowed(borrowed) => Self::Borrowed(borrowed),
            MaybeOwnedString::Owned(owned) => Self::Owned(owned)
        }
    }
}

#[cfg(feature = "serde")]
impl<'a, 'de: 'a> serde::de::Deserialize<'de> for MaybeOwnedString<'a> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: serde::de::Deserializer<'de>, {
        use serde::de::Error;
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = MaybeOwnedString<'de>;

            fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
                formatter.write_str("a string")
            }

            fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E> where E: Error {
                Ok(MaybeOwnedString::Borrowed(value))
            }
            fn visit_string<E>(self, value: String) -> Result<Self::Value, E> where E: Error {
                Ok(MaybeOwnedString::Owned(value))
            }
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> where E: Error {
                Ok(MaybeOwnedString::Owned(value.to_string()))
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}
#[cfg(feature = "serde")]
impl serde::Serialize for MaybeOwnedString<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self)
    }
}

/// A wrapper struct to enforce deserialization into an owned string rather than attempt to borrow from the source data.
#[cfg(feature = "serde")] #[repr(transparent)] #[derive(Debug, Clone,PartialEq, Eq, PartialOrd, Ord)] pub struct MaybeOwnedStringDeserializeToOwned<'a>(pub MaybeOwnedString<'a>);
#[cfg(feature = "serde")] impl<'a> MaybeOwnedStringDeserializeToOwned<'a> {
    pub fn new(value: impl Into<MaybeOwnedString<'a>>) -> Self {
        Self(value.into())
    }
    pub const fn owned(string: String) -> Self {
        Self(MaybeOwnedString::Owned(string))
    }
    pub const fn borrowed(str: &'a str) -> Self {
        Self(MaybeOwnedString::Borrowed(str))
    }
    pub fn into_inner(self) -> MaybeOwnedString<'a> {
        self.0
    }
}
#[cfg(feature = "serde")] impl<'a> core::ops::Deref for MaybeOwnedStringDeserializeToOwned<'a> {
    type Target = MaybeOwnedString<'a>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
#[cfg(feature = "serde")] impl core::ops::DerefMut for MaybeOwnedStringDeserializeToOwned<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
#[cfg(feature = "serde")] impl<'a> AsRef<MaybeOwnedString<'a>> for MaybeOwnedStringDeserializeToOwned<'a> {
    fn as_ref(&self) -> &MaybeOwnedString<'a> {
        &self.0
    }
}
#[cfg(feature = "serde")]
impl<'de> serde::de::Deserialize<'de> for MaybeOwnedStringDeserializeToOwned<'_> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: serde::de::Deserializer<'de>, {
        use serde::de::Error;
        struct Visitor<'a>(core::marker::PhantomData<&'a ()>);
        impl<'a> serde::de::Visitor<'_> for Visitor<'a> {
            type Value = MaybeOwnedStringDeserializeToOwned<'a>;

            fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
                formatter.write_str("a string")
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E> where E: Error {
                Ok(MaybeOwnedStringDeserializeToOwned(MaybeOwnedString::Owned(value)))
            }
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> where E: Error {
                Ok(MaybeOwnedStringDeserializeToOwned(MaybeOwnedString::Owned(value.to_string())))
            }
        }

        deserializer.deserialize_string(Visitor(core::marker::PhantomData))
    }
}
#[cfg(feature = "serde")]
impl serde::Serialize for MaybeOwnedStringDeserializeToOwned<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self)
    }
}
#[cfg(feature = "serde")] impl AsRef<str> for MaybeOwnedStringDeserializeToOwned<'_> {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}



