use std::hash::Hash;
use std::marker::PhantomData;

pub use persistent::Id as PersistentId;
pub mod persistent {
    use super::*;

    /// A persistent ID is an ID for an entity stored within the database.
    /// It is unchanging over time, and is always present for an entity, no matter if it's local or cloud-based.
    /// 
    /// TBD: Is it shared per-machine or per-cloud-sync?
    pub struct Id<T>(u64, PhantomData<T>);
    impl<T> Id<T> {
        // todo: ctor should be unsafe (cuz not positively present or tied to type)
        
        pub fn from_hex(value: &str) -> Result<Self, core::num::ParseIntError> {
            Ok(Id::new(u64::from_str_radix(value, 16)?))       
        }


        pub fn new(raw: u64) -> Self { Self(raw, PhantomData) }

        pub fn get_raw(&self) -> u64 {
            self.0
        }
    }
    impl<T> Clone for Id<T> {
        fn clone(&self) -> Self { *self }
    }
    impl<T> Copy for Id<T> {}
    impl<T> PartialEq for Id<T> {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }
    impl<T> Eq for Id<T> {}
    impl<T> Hash for Id<T> {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            state.write_u64(self.0);
        }
    }
    impl<T> From<u64> for Id<T> {
        fn from(value: u64) -> Self {
            Self::new(value)
        }
    }
    impl<T> From<Id<T>> for u64 {
        fn from(val: Id<T>) -> Self {
            val.0
        }
    }
    impl<T> TryFrom<&str> for Id<T> {
        type Error = core::num::ParseIntError;
        fn try_from(value: &str) -> Result<Self, Self::Error> {
            Ok(Id::new(u64::from_str_radix(value, 16)?))
        }
    }
    impl<T: Possessor> core::fmt::Debug for Id<T> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.pad(&format!("PersistentId<{:?}>({})", T::IDENTITY, self.0))
        }
    }

    pub trait Possessor {
        type Id: Clone + Copy + Hash + PartialEq + Eq;
        #[allow(private_interfaces)]
        const IDENTITY: PossessorIdentity;
        fn get_persistent_id(&self) -> Self::Id;
    }

    #[derive(Debug)]
    pub(crate) enum PossessorIdentity {
        Track,
        Account,
        Artist,
        Album,
        Collection
    }
}

pub mod cloud {
    use super::*;

    pub use library::Id as Library;
    pub mod library {
        use serde::{Serialize, Deserialize};
        use super::*;

        #[derive(Debug, PartialEq, Eq)]
        pub struct BadNamespaceError;
        impl core::fmt::Display for BadNamespaceError {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str("Namespace does not match the associated type")
            }
        }

        /// A cloud library resource-typed ID is an ID for a library entity (one which is scoped to the user) stored within the cloud.
        /// It is a short string prefixed with one to two letters (the namespace) and a full stop.
        // TODO: Document local-synced IDs. ("l.z-")
        pub struct Id<
            T, // possessor type,
            S, // string type
        >(S, PhantomData<T>);
        impl<T, S> Id<T, S> where S: AsRef<str> {
            pub fn new(value: S) -> Result<Self, BadNamespaceError> where T: Possessor {
                // uhh this doesn't check for the full stop but it's ok i guess
                if value.as_ref().starts_with(T::IDENTITY.into_namespace()) {
                    Ok(Self(value, PhantomData))
                } else {
                    Err(BadNamespaceError)
                }
            }
        }
        impl<T, S> Id<T, S> {
            /// # Safety
            /// The caller must ensure that the string is a valid cloud library ID.
            /// The associated type must match the ID namespace.
            pub unsafe fn new_unchecked(value: S) -> Self {
                Self(value, PhantomData)
            }
        }
        impl<T, S> Clone for Id<T, S> where S: Clone {
            fn clone(&self) -> Self {
                Self(self.0.clone(), PhantomData)
            }
        }
        impl<T, S> Copy for Id<T, S> where S: Copy {}
        impl<T, S> PartialEq for Id<T, S> where S: PartialEq {
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }
        impl<T, S> Eq for Id<T, S> where S: Eq {}
        impl<T, S> Hash for Id<T, S> where S: Hash {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.0.hash(state);
            }
        }
        impl<T, S> core::fmt::Display for Id<T, S> where S: core::fmt::Display {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                self.0.fmt(f)
            }
        }
        impl<T, S> core::fmt::Debug for Id<T, S> where S: core::fmt::Debug, T: Possessor {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.pad(&format!("CloudLibraryId<{:?}>({:?})", T::IDENTITY, self.0))
            }
        }
        
        impl<T, S> Serialize for Id<T, S> where S: Serialize {
            fn serialize<SR: serde::Serializer>(&self, serializer: SR) -> Result<SR::Ok, SR::Error> {
                self.0.serialize(serializer)
            }
        }
        impl<'de, T, S> Deserialize<'de> for Id<T, S> where T: Possessor, S: Deserialize<'de> + AsRef<str> {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = S::deserialize(deserializer)?;
                Self::new(value).map_err(serde::de::Error::custom)
            }
        }

        pub trait Possessor {
            #[allow(private_interfaces)]
            const IDENTITY: PossessorIdentity;
        }

        #[derive(Debug, Clone, Copy)]
        pub(crate) enum PossessorIdentity {
            Track,
            Account,
            Artist,
            Album,
            Collection
        }
        impl PossessorIdentity {
            pub const fn into_namespace(self) -> &'static str {
                match self {
                    Self::Track => "i",
                    Self::Account => "sp",
                    Self::Artist => "r",
                    Self::Album => "l",
                    Self::Collection => "p"
                }
            }
        }
    }

    pub use catalog::Id as Catalog;
    pub mod catalog {
        use core::num::NonZeroU32;
        use super::*;

        /// A cloud catalog ID is a 32-bit unsigned integer pointing to a resource in the cloud (specifically those provided by Apple Music?).
        pub struct Id<T>(NonZeroU32, PhantomData<T>);
        impl<T> Id<T> {
            /// # Safety
            /// The caller must ensure that the value is a valid cloud catalog ID.
            /// The associated type must match what the ID really points to.
            pub unsafe fn new_unchecked(value: NonZeroU32) -> Self {
                Self(value, PhantomData)
            }

            pub fn get_raw(&self) -> u32 {
                self.0.get()
            }
        }
        impl<T> PartialEq for Id<T> {
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }
        impl<T> Eq for Id<T> {}
        impl<T> Clone for Id<T> {
            fn clone(&self) -> Self { *self }
        }
        impl<T> Copy for Id<T> {}
        impl<T> Hash for Id<T> {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                state.write_u32(self.0.get());
            }
        }
        impl<T> core::fmt::Display for Id<T> {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                self.0.fmt(f)
            }
        }
        impl<T> core::fmt::Debug for Id<T> where T: Possessor {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.pad(&format!("CloudCatalogId<{:?}>({})", T::IDENTITY, self.0))
            }
        }
        impl<T> serde::Serialize for Id<T> {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                self.0.serialize(serializer)
            }
        }
        impl<T> From<Id<T>> for u32 {
            fn from(val: Id<T>) -> Self {
                val.0.get()
            }
        }


        pub trait Possessor {
            #[allow(private_interfaces)]
            const IDENTITY: PossessorIdentity;
        }

        #[derive(Debug, Clone, Copy)]
        pub(crate) enum PossessorIdentity {
            Track,
            Artist,
            Album,
            Collection
        }
    }
}
