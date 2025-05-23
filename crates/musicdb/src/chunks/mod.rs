mod album;
mod artist;
mod track;
mod account;
mod collection;
pub use album::*;
pub use artist::*;
pub use track::*;
pub use account::*;
pub use collection::*;

derive_list!(pub LibraryMaster, crate::Boma<'a>, *b"plma");

use std::marker::PhantomData;

use crate::{id, setup_eaters, chunk::*};

#[derive(Debug)]
pub struct SectionBoundary<T>  {
    // r0x0..3 ; b"hsma"
    // boundary_length: u32, // r0x4..7
    // section_length: u32, // r0x8..12
    _subtype: T, // r0x12..15
    // ; ...zeros, len-12
}
impl<T> Chunk for SectionBoundary<T> {
    const SIGNATURE: Signature = crate::chunk::Signature::new(*b"hsma");
}
impl<T: From<u32>> SizedFirstReadableChunk<'_> for SectionBoundary<T> {
    type ReadError = std::io::Error;

    fn read_sized_content(cursor: &mut std::io::Cursor<&'_ [u8]>, offset: u64, length: u32) -> Result<Self, Self::ReadError> {
        setup_eaters!(cursor, offset, length);
        skip!(4)?; // len of section
        let subtype = T::from(u32!()?);
        skip_to_end!()?;
        Ok(Self { _subtype: subtype })
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ListReadError<T: core::fmt::Debug> {
    #[error("bad item: {0}")]
    BadItem(T),
    #[error("bad list header: {0}")]
    BadListHeader(std::io::Error),
}
impl<T: core::fmt::Debug> From<std::io::Error> for ListReadError<T> {
    fn from(value: std::io::Error) -> Self {
        Self::BadListHeader(value)
    }
}


pub struct List<'a, T>(pub Vec<T>, PhantomData<&'a ()>);
#[allow(private_bounds)]
impl<'a, T: ReadableChunk<'a>> List<'a, T> {
    pub(crate) fn read_contents(cursor: &mut std::io::Cursor<&'a [u8]>, _: u64, length: u32) -> Result<Self, ListReadError<<T as ReadableChunk<'a>>::ReadError>> {
        setup_eaters!(cursor, offset, length);
        let item_count = u32!().map_err(ListReadError::BadListHeader)? as usize;
        // dbg!(offset, length, item_count);
        cursor.advance(length as i64 - 12).map_err(ListReadError::BadListHeader)?;

        
        let mut items = Vec::with_capacity(item_count);
        for item in cursor.reading_chunks::<T>(item_count) {
            items.push(item.map_err(ListReadError::BadItem)?);
        }
        Ok(Self(items, PhantomData))
    }
}
impl<'a, T: ReadableChunk<'a>> core::fmt::Debug for List<'a, T> where T: core::fmt::Debug {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("List").field(&self.0).finish()
    }
}
impl<'a, T: ReadableChunk<'a>> core::ops::Deref for List<'a, T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<'a, T: ReadableChunk<'a>> IntoIterator for List<'a, T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

use std::collections::HashMap;

pub struct Map<'a, T: id::persistent::Possessor>(pub HashMap<T::Id, T>, PhantomData<&'a ()>);
impl<'a, T: ReadableChunk<'a> + id::persistent::Possessor> Map<'a, T> {
    pub(crate) fn read_contents(cursor: &mut std::io::Cursor<&'a [u8]>, offset: u64, length: u32) -> Result<Self, ListReadError<<T as ReadableChunk<'a>>::ReadError>> where <T as id::persistent::Possessor>::Id: core::fmt::Debug {
        setup_eaters!(cursor, offset, length);
        let item_count = u32!().map_err(ListReadError::BadListHeader)? as usize;
        skip_to_end!().map_err(ListReadError::BadListHeader)?;
        let mut items = HashMap::<T::Id, T>::with_capacity(item_count);
        for item in cursor.reading_chunks::<T>(item_count) {
            let item = item.map_err(ListReadError::BadItem)?;
            items.insert(item.get_persistent_id(), item);
        }
        Ok(Self(items, PhantomData))
    }
}
impl<'a, T: ReadableChunk<'a> + id::persistent::Possessor> core::fmt::Debug for Map<'a, T> where T: core::fmt::Debug, T::Id: core::fmt::Debug  {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Map ")?;
        f.debug_map().entries(self.iter()).finish()
    }
}
impl<'a, T: ReadableChunk<'a> + id::persistent::Possessor> core::ops::Deref for Map<'a, T> {
    type Target = HashMap<T::Id, T>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<'a, T: ReadableChunk<'a> + id::persistent::Possessor> IntoIterator for Map<'a, T> {
    type Item = (T::Id, T);
    type IntoIter = std::collections::hash_map::IntoIter<T::Id, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
impl<'a, T: ReadableChunk<'a> + id::persistent::Possessor> IntoIterator for &Map<'a, T> where Self: 'a {
    type Item = (&'a T::Id, &'a T);
    type IntoIter = std::collections::hash_map::Iter<'a, T::Id, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

#[macro_export]
macro_rules! derive_list {
    ($vis: vis $identifier: ident, $content: ty, $signature: expr) => {
        $vis type $identifier<'a> = $crate::chunks::List<'a, $content>;

        impl<'a> $crate::chunk::Chunk for $identifier<'a> {
            const SIGNATURE: $crate::chunk::Signature = $crate::chunk::Signature::new($signature);
        }
        impl<'a> $crate::chunk::SizedFirstReadableChunk<'a> for $identifier<'a> {
            type ReadError = $crate::chunks::ListReadError<<$content as $crate::chunk::ReadableChunk<'a>>::ReadError>;

            fn read_sized_content(cursor: &mut std::io::Cursor<&'a [u8]>, offset: u64, length: u32) -> Result<Self, Self::ReadError> {
                Ok(Self($crate::chunks::List::read_contents(cursor, offset, length)?.0, ::core::marker::PhantomData))
            }
        }
    }
}

pub use derive_list;

#[macro_export]
macro_rules! derive_map {
    ($vis: vis $identifier: ident, $content: ty, $signature: expr) => {
        $vis type $identifier<'a> = $crate::chunks::Map<'a, $content>;

        impl<'a> $crate::chunk::Chunk for $identifier<'a> {
            const SIGNATURE: $crate::chunk::Signature = $crate::chunk::Signature::new($signature);
        }
        impl<'a> $crate::chunk::SizedFirstReadableChunk<'a> for $identifier<'a> {
            type ReadError = $crate::chunks::ListReadError<<$content as $crate::chunk::ReadableChunk<'a>>::ReadError>;

            fn read_sized_content(cursor: &mut std::io::Cursor<&'a [u8]>, offset: u64, length: u32) -> Result<Self, Self::ReadError> {
                Ok(Self($crate::chunks::Map::read_contents(cursor, offset, length)?.0, ::core::marker::PhantomData))
            }
        }
    }
}

pub use derive_map;

