use crate::{endian::EndianIdentifier, UnalignedU16Slice};

pub mod error {
    /// An error indicating that a non-character was encountered
    #[derive(Debug)]
    pub struct NonCharacterEncountered;
    impl core::error::Error for NonCharacterEncountered {}
    impl core::fmt::Display for NonCharacterEncountered {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "non-character encountered")
        }
    }

    /// An error indicating that an unpaired surrogate was present
    #[derive(Debug)]
    pub struct UnpairedSurrogate;
    impl core::error::Error for UnpairedSurrogate {}
    impl core::fmt::Display for UnpairedSurrogate {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "unpaired surrogate")
        }
    }
        
    #[derive(thiserror::Error, Debug)]
    pub enum InvalidUtf16 {
        #[error("{0}")]
        BadByteLength(#[from] crate::error::BadByteLength),
        #[error("{0}")]
        UnpairedSurrogate(#[from] UnpairedSurrogate),
        #[error("{0}")]
        NonCharacterEncountered(#[from] NonCharacterEncountered)
    }
}

#[allow(private_bounds)]
#[repr(transparent)]
pub struct Utf16Str<T: EndianIdentifier = ()> {
    _endianness: core::marker::PhantomData<T>,
    slice: UnalignedU16Slice,
}
#[allow(private_bounds)]
impl<'a, T: EndianIdentifier> Utf16Str<T> {
    pub fn new(slice: impl TryInto<&'a UnalignedU16Slice, Error = crate::error::BadByteLength>) -> Result<&'a Self, error::InvalidUtf16> {
        let slice: &UnalignedU16Slice = slice.try_into()?;
        // TODO: Check validity.
        Ok(unsafe { Self::new_unchecked(slice.bytes()) })
    }

    /// # Safety
    /// - The provided slice must have a length that is a multiple of two.
    /// - The contents of the slice must be valid UTF-16.
    pub unsafe fn new_unchecked(slice: &'a [u8]) -> &'a Self {
        unsafe { &*(slice as *const [u8] as *const Self) }
    }

    /// Returns the length of the string in bytes.
    pub fn len(&self) -> usize {
        self.slice.byte_len()
    }

    /// Returns true if the string is empty.
    pub fn is_empty(&self) -> bool {
        self.slice.is_empty()
    }

    pub fn bytes(&'a self) -> &'a [u8] {
        self.slice.bytes()
    }

    /// Returns an [`UnalignedU16Slice`] of this data.
    pub fn unaligned_shorts(&self) -> &UnalignedU16Slice {
        unsafe { UnalignedU16Slice::new_unchecked(self.bytes()) }
    }

    /// Returns an iterator over the characters of the string.
    pub fn chars(&'a self) -> iter::UnalignedUtf16StrCharacterIterator<'a> {
        iter::UnalignedUtf16StrCharacterIterator::new(self)
    }

    pub fn starts_with(&self, prefix: impl traits::starts_with::PrefixCheck) -> bool {
        prefix.is_prefix_of(self)
    }
    
    pub fn utf8_byte_len(&self) -> usize {
        self.chars().map(|char| char.len_utf8()).sum()
    }
}
impl<T: EndianIdentifier> PartialEq<str> for Utf16Str<T> {
    fn eq(&self, other: &str) -> bool {
        let mut utf8_chars = other.chars();
        let mut utf16_chars = self.chars();

        loop {
            match (utf16_chars.next(), utf8_chars.next()) {
                (Some(lhs), Some(rhs)) => if lhs != rhs { return false }
                (None, None) => return true,
                _ => return false,
            }
        }
    }
}
impl<T: EndianIdentifier> PartialEq<str> for &Utf16Str<T> {
    fn eq(&self, other: &str) -> bool {
        (*self).eq(other)
    }
}
impl<T: EndianIdentifier> PartialEq<&str> for Utf16Str<T> {
    fn eq(&self, other: &&str) -> bool {
        self.eq(*other)
    }
}
impl<T: EndianIdentifier, U: EndianIdentifier> PartialEq<Utf16Str<U>> for Utf16Str<T> {
    fn eq(&self, other: &Utf16Str<U>) -> bool {
        self.bytes() == other.bytes()
    }
}
impl<T: EndianIdentifier, U: EndianIdentifier> PartialEq<&Utf16Str<U>> for Utf16Str<T> {
    fn eq(&self, other: &&Utf16Str<U>) -> bool {
        self.eq(*other)
    }
}
impl<T: EndianIdentifier, U: EndianIdentifier> PartialEq<Utf16Str<U>> for &Utf16Str<T> {
    fn eq(&self, other: &Utf16Str<U>) -> bool {
        (*self).eq(other)
    }
}
impl<T: EndianIdentifier> Eq for Utf16Str<T> {}
impl<T: EndianIdentifier> Ord for Utf16Str<T> {
    fn cmp(&self, other: &Utf16Str<T>) -> std::cmp::Ordering {
        unsafe {
            <Self as core::cmp::PartialOrd>::partial_cmp(self, other)
                .unwrap_unchecked()
        }
    }
}
impl<T: EndianIdentifier> PartialOrd<str> for Utf16Str<T> {
    fn partial_cmp(&self, other: &str) -> Option<std::cmp::Ordering> {
        let mut utf8_chars = other.chars();
        let mut utf16_chars = self.chars();
        use core::cmp::Ordering;
        let mut cmp  = Ordering::Equal;
        loop {
            cmp = match cmp {
                Ordering::Equal => {
                    let lhs = utf16_chars.next();
                    let rhs = utf8_chars.next();
                    if lhs.is_none() && rhs.is_none() { return Some(Ordering::Equal) }
                    lhs.partial_cmp(&rhs)?
                },
                ordering => return Some(ordering)
            }
        }
    }
}
impl<T: EndianIdentifier> PartialOrd<str> for &Utf16Str<T> {
    fn partial_cmp(&self, other: &str) -> Option<std::cmp::Ordering> {
        (*self).partial_cmp(other)
    }
}
impl<T: EndianIdentifier> PartialOrd<&str> for Utf16Str<T> {
    fn partial_cmp(&self, other: &&str) -> Option<std::cmp::Ordering> {
        self.partial_cmp(*other)
    }
}
impl<T: EndianIdentifier, U: EndianIdentifier> PartialOrd<Utf16Str<U>> for Utf16Str<T> {
    fn partial_cmp(&self, other: &Utf16Str<U>) -> Option<std::cmp::Ordering> {
        let mut lhs_chars = self.chars();
        let mut rhs_chars = other.chars();
        use core::cmp::Ordering;
        let mut cmp = Ordering::Equal;
        loop {
            cmp = match cmp {
                Ordering::Equal => {
                    let lhs = lhs_chars.next();
                    let rhs = rhs_chars.next();
                    lhs.cmp(&rhs)
                },
                ordering => return Some(ordering)
            }
        }
    }
}
impl<T: EndianIdentifier, U: EndianIdentifier> PartialOrd<&Utf16Str<U>> for Utf16Str<T> {
    fn partial_cmp(&self, other: &&Utf16Str<U>) -> Option<std::cmp::Ordering> {
        self.partial_cmp(*other)
    }
}
impl<T: EndianIdentifier, U: EndianIdentifier> PartialOrd<Utf16Str<U>> for &Utf16Str<T> {
    fn partial_cmp(&self, other: &Utf16Str<U>) -> Option<std::cmp::Ordering> {
        (*self).partial_cmp(other)
    }
}
impl<T: EndianIdentifier> core::hash::Hash for Utf16Str<T> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.bytes().hash(state)
    }
}
impl<T: EndianIdentifier> core::fmt::Display for Utf16Str<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        use core::fmt::Write;
        for char in self.chars() {
            f.write_char(char)?;
        }
        Ok(())
    }
}
impl<T: EndianIdentifier> core::fmt::Debug for Utf16Str<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if f.sign_minus() {
            f.debug_struct("Utf16Str")
                .field("bytes", &self.bytes())
                .finish()
        } else {
            f.debug_tuple("Utf16Str")
                .field(&self.to_string())
                .finish()
        }
    }
}

impl<T: EndianIdentifier> From<&Utf16Str<T>> for String {
    fn from(val: &Utf16Str<T>) -> Self {
        let mut string = String::with_capacity(val.utf8_byte_len());
        for char in val.chars() {
            string.push(char);
        }
        string
    }
}

impl<T: EndianIdentifier> PartialEq<Utf16Str<T>> for str {
    fn eq(&self, other: &Utf16Str<T>) -> bool {
        other == self
    }
}
impl<T: EndianIdentifier> PartialEq<Utf16Str<T>> for &str {
    fn eq(&self, other: &Utf16Str<T>) -> bool {
        other == *self
    }
}
impl<T: EndianIdentifier> PartialEq<Utf16Str<T>> for dyn AsRef<str> {
    fn eq(&self, other: &Utf16Str<T>) -> bool {
        self.as_ref() == other
    }
}
impl<T: EndianIdentifier> PartialOrd<Utf16Str<T>> for str {
    fn partial_cmp(&self, other: &Utf16Str<T>) -> Option<std::cmp::Ordering> {
        other.partial_cmp(self).map(|ordering| ordering.reverse())
    }
}
impl<T: EndianIdentifier> PartialOrd<Utf16Str<T>> for &str {
    fn partial_cmp(&self, other: &Utf16Str<T>) -> Option<std::cmp::Ordering> {
        other.partial_cmp(*self).map(|ordering| ordering.reverse())
    }
}
impl<T: EndianIdentifier> PartialOrd<Utf16Str<T>> for dyn AsRef<str> {
    fn partial_cmp(&self, other: &Utf16Str<T>) -> Option<std::cmp::Ordering> {
        self.as_ref().partial_cmp(other).map(|ordering| ordering.reverse())
    }
}


pub mod iter {
    use crate::endian::EndianIdentifier;

    /// An iterator over the characters of a UTF-16 string.
    /// 
    /// In debug mode, panics if an invalid character is encountered.
    /// In release mode, assumes all characters are valid.
    /// 
    /// Does not implement [`core::iter::ExactSizeIterator`] because UTF-16 characters are not fixed-width, and this iterator is intentionally lazy.
    pub struct UnalignedUtf16StrCharacterIterator<'a> {
        inner: core::char::DecodeUtf16<crate::iter::UnalignedU16SliceIterator<'a>>
    }

    impl<'a> UnalignedUtf16StrCharacterIterator<'a> {
        pub fn new<T: EndianIdentifier>(str: &'a super::Utf16Str<T>) -> Self {
            Self {
                inner: core::char::decode_utf16(str.unaligned_shorts().iter(T::to_variant()))
            }
        }
    }
    impl Iterator for UnalignedUtf16StrCharacterIterator<'_> {
        type Item = char;
        fn next(&mut self) -> Option<Self::Item> {
            let result = self.inner.next()?;
            Some({
                #[cfg(debug_assertions)]
                { result.expect("invalid character encountered") }
                #[cfg(not(debug_assertions))]
                unsafe { result.unwrap_unchecked() }
            })
        }
        
        fn size_hint(&self) -> (usize, Option<usize>) {
            self.inner.size_hint()
        }
    }
    impl core::iter::FusedIterator for UnalignedUtf16StrCharacterIterator<'_> {}
}

pub mod traits {
    use super::*;

    pub mod starts_with {
        use super::*;


        pub trait PrefixCheck {
            /// Returns true if `T` starts with `self`.
            /// Doesn't do any character normalization.
            fn is_prefix_of<T: EndianIdentifier>(&self, against: &Utf16Str<T>) -> bool;
        }
    

        impl PrefixCheck for str {
            fn is_prefix_of<T: EndianIdentifier>(&self, against: &Utf16Str<T>) -> bool {
                <dyn AsRef<str> as PrefixCheck>::is_prefix_of(&self, against)
            }
        }


        impl PrefixCheck for &str {
            fn is_prefix_of<T: EndianIdentifier>(&self, against: &Utf16Str<T>) -> bool {
                <dyn AsRef<str> as PrefixCheck>::is_prefix_of(&self, against)
            }
        }


        impl PrefixCheck for dyn AsRef<str> + '_ {
            fn is_prefix_of<T: EndianIdentifier>(&self, against: &Utf16Str<T>) -> bool {
                let mut utf8_chars = self.as_ref().chars();
                let mut utf16_chars = against.chars();
    
                loop {
                    match (utf8_chars.next(), utf16_chars.next()) {
                        (Some(lhs), Some(rhs)) => if lhs != rhs { return false }
                        (None, None) => return true,
                        _ => return false,
                    }
                }
            }
        }


        impl<T: EndianIdentifier> PrefixCheck for Utf16Str<T> {
            fn is_prefix_of<U: EndianIdentifier>(&self, against: &Utf16Str<U>) -> bool {
                self.bytes().starts_with(against.bytes())
            }
        }
    }
}


#[macro_export]
macro_rules! utf16_literal {
    ($v: expr) => {
        $crate::utf16::Utf16Str::<$crate::endian::SystemEndian>::new($crate::u16_slice_as_u8_slice(&$v.encode_utf16().collect::<Vec<_>>())).expect("bad utf16")
    };
}

pub use utf16_literal;

#[cfg(test)]
mod tests {
    use crate::utf16_literal as utf16;

    #[test]
    fn eq() {
        assert!(utf16!("jor") == "jor");
        assert!(utf16!("👨‍👩‍👧‍👦") == "👨‍👩‍👧‍👦");
        assert!(utf16!("🙃") == "🙃");
        assert!(utf16!("🧑🏾‍❤️‍💋‍🧑🏻") == "🧑🏾‍❤️‍💋‍🧑🏻");
    }

    #[test]
    fn cmp() {
        macro_rules! test_equiv {
            ($(($lhs: literal, $rhs: literal) $(,)?)*) => {
                $(
                    assert_eq!(utf16!($lhs).partial_cmp(utf16!($rhs)), $lhs.partial_cmp($rhs));
                    assert_eq!(utf16!($lhs).partial_cmp($rhs), $lhs.partial_cmp(utf16!($rhs)));
                )*
            };
        }

        test_equiv!(
            ("string", "rope"),
            ("👨‍👩‍👧‍👦", "🙃"),
            ("📻", "3"),
        );
    }
}
