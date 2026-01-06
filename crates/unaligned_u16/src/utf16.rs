use core::cmp::Ordering;

use crate::{UnalignedU16Slice, endian::{Endian, Endianness}};

impl Endianness {
    /// Determines the endianness from a byte order mark (BOM) at the start of a byte slice.
    #[must_use]
    pub const fn from_bom(bytes: &[u8]) -> Option<Self> {
        match bytes {
            [0xFF, 0xFE, ..] => Some(Self::Little),
            [0xFE, 0xFF, ..] => Some(Self::Big),
            _ => None,
        }
    }
}

/// Validation errors that can occur when constructing the [`Utf16Str`] data abstraction.
#[derive(thiserror::Error, Debug)]
pub enum InvalidUtf16Error {
    #[error("{0}")]
    BadByteLength(#[from] crate::error::BadByteLength),
    #[error("{0}")]
    IncorrectEncoding(#[from] core::char::DecodeUtf16Error),
}

/// A UTF-16 encoded string with unaligned u16 data.
/// 
/// # Endianness
/// 
/// The endianness of the UTF-16 string is determined by the type parameter `T`.
/// Byte order markers are not handled or respected; the caller is responsible for ensuring the correct endianness.
#[allow(private_bounds)]
#[repr(transparent)]
pub struct Utf16Str<T: Endian> {
    _endianness: core::marker::PhantomData<T>,
    slice: UnalignedU16Slice,
}
#[allow(private_bounds)]
impl<'a, T: Endian> Utf16Str<T> {
    /// Creates a new UTF-16 string from the provided byte slice.
    /// Returns an error if the byte slice length is not a multiple of two, or if the contents are not valid UTF-16.
    /// 
    /// # Errors
    /// - [`crate::error::BadByteLength`] if the length of the slice is not a multiple of two.
    /// - [`InvalidUtf16Error`] if the contents of the slice are not valid UTF-16.
    pub fn new(slice: impl TryInto<&'a UnalignedU16Slice, Error = crate::error::BadByteLength>) -> Result<&'a Self, InvalidUtf16Error> {
        let slice: &UnalignedU16Slice = slice.try_into()?;
        for result in char::decode_utf16(slice.iter(T::to_variant())) { result?; }
        Ok(unsafe { Self::new_unchecked(slice.bytes()) })
    }

    /// # Safety
    /// - The provided slice must have a length that is a multiple of two.
    /// - The contents of the slice must be valid UTF-16.
    #[must_use]
    pub const unsafe fn new_unchecked(slice: &'a [u8]) -> &'a Self {
        unsafe { &*(core::ptr::from_ref::<[u8]>(slice) as *const Self) }
    }

    /// Whether the string is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.slice.is_empty()
    }

    /// The underlying byte slice of the string.
    #[must_use]
    pub const fn bytes(&'a self) -> &'a [u8] {
        self.slice.bytes()
    }

    /// Returns an [`UnalignedU16Slice`] of this data.
    #[must_use]
    pub const fn unaligned_shorts(&self) -> &UnalignedU16Slice {
        &self.slice
    }

    /// Returns an iterator over the characters of the string.
    #[must_use]
    pub fn chars(&'a self) -> iter::UnalignedUtf16StrCharacterIterator<'a> {
        iter::UnalignedUtf16StrCharacterIterator::new(self)
    }

    /// Whether this string starts with the given prefix.
    pub fn starts_with(&self, prefix: &impl traits::starts_with::PrefixChecker) -> bool {
        prefix.is_prefix_of(self)
    }
    
    /// How many bytes this string would take up if encoded as UTF-8.
    #[must_use]
    pub fn utf8_byte_len(&self) -> usize {
        self.chars().map(char::len_utf8).sum()
    }
}
impl<T: Endian> PartialEq<str> for Utf16Str<T> {
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
impl<T: Endian> PartialEq<str> for &Utf16Str<T> {
    fn eq(&self, other: &str) -> bool {
        (*self).eq(other)
    }
}
impl<T: Endian> PartialEq<&str> for Utf16Str<T> {
    fn eq(&self, other: &&str) -> bool {
        self.eq(*other)
    }
}
impl<T: Endian, U: Endian> PartialEq<Utf16Str<U>> for Utf16Str<T> {
    fn eq(&self, other: &Utf16Str<U>) -> bool {
        if T::IS_LITTLE == U::IS_LITTLE {
            // TODO: At the very least, maybe we can ignore a BOM here?
            self.slice.bytes() == other.slice.bytes()
        } else {
            let mut lhs_chars = self.chars();
            let mut rhs_chars = other.chars();

            loop {
                let lhs = lhs_chars.next();
                let rhs = rhs_chars.next();
                if lhs.is_none() && rhs.is_none() { return true }
                if lhs != rhs { return false }
            }
        }
    }
}
impl<T: Endian, U: Endian> PartialEq<&Utf16Str<U>> for Utf16Str<T> {
    fn eq(&self, other: &&Utf16Str<U>) -> bool {
        self.eq(*other)
    }
}
impl<T: Endian, U: Endian> PartialEq<Utf16Str<U>> for &Utf16Str<T> {
    fn eq(&self, other: &Utf16Str<U>) -> bool {
        (*self).eq(other)
    }
}
impl<T: Endian> Eq for Utf16Str<T> {}
impl<T: Endian> Ord for Utf16Str<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        unsafe {
            <Self as core::cmp::PartialOrd>::partial_cmp(self, other)
                .unwrap_unchecked()
        }
    }
}
impl<T: Endian> PartialOrd<str> for Utf16Str<T> {
    fn partial_cmp(&self, other: &str) -> Option<Ordering> {
        let mut utf8_chars = other.chars();
        let mut utf16_chars = self.chars();
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
impl<T: Endian> PartialOrd<str> for &Utf16Str<T> {
    fn partial_cmp(&self, other: &str) -> Option<Ordering> {
        (*self).partial_cmp(other)
    }
}
impl<T: Endian> PartialOrd<&str> for Utf16Str<T> {
    fn partial_cmp(&self, other: &&str) -> Option<Ordering> {
        self.partial_cmp(*other)
    }
}
impl<T: Endian, U: Endian> PartialOrd<Utf16Str<U>> for Utf16Str<T> {
    fn partial_cmp(&self, other: &Utf16Str<U>) -> Option<Ordering> {
        let mut lhs_chars = self.chars();
        let mut rhs_chars = other.chars();
        let mut ordering = Ordering::Equal;
        while ordering == Ordering::Equal {
            let lhs = lhs_chars.next();
            let rhs = rhs_chars.next();
            ordering = lhs.cmp(&rhs);
        }
        Some(ordering)
    }
}
impl<T: Endian, U: Endian> PartialOrd<&Utf16Str<U>> for Utf16Str<T> {
    fn partial_cmp(&self, other: &&Utf16Str<U>) -> Option<Ordering> {
        self.partial_cmp(*other)
    }
}
impl<T: Endian, U: Endian> PartialOrd<Utf16Str<U>> for &Utf16Str<T> {
    fn partial_cmp(&self, other: &Utf16Str<U>) -> Option<Ordering> {
        (*self).partial_cmp(other)
    }
}
impl<T: Endian> core::hash::Hash for Utf16Str<T> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.bytes().hash(state);
    }
}
impl<T: Endian> core::fmt::Display for Utf16Str<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        use core::fmt::Write;
        for char in self.chars() {
            f.write_char(char)?;
        }
        Ok(())
    }
}
impl<T: Endian> core::fmt::Debug for Utf16Str<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if f.sign_minus() {
            f.debug_struct("Utf16Str")
                .field("bytes", &self.bytes())
                .finish()
        } else {
            f.debug_tuple("Utf16Str")
                .field(&self)
                .finish()
        }
    }
}

#[cfg(feature = "alloc")]
impl<T: Endian> From<&Utf16Str<T>> for alloc::string::String {
    fn from(val: &Utf16Str<T>) -> Self {
        let mut string = Self::with_capacity(val.utf8_byte_len());
        for char in val.chars() {
            string.push(char);
        }
        string
    }
}

impl<T: Endian> PartialEq<Utf16Str<T>> for str {
    fn eq(&self, other: &Utf16Str<T>) -> bool {
        other == self
    }
}
impl<T: Endian> PartialEq<Utf16Str<T>> for &str {
    fn eq(&self, other: &Utf16Str<T>) -> bool {
        other == *self
    }
}
impl<T: Endian> PartialEq<Utf16Str<T>> for dyn AsRef<str> {
    fn eq(&self, other: &Utf16Str<T>) -> bool {
        self.as_ref() == other
    }
}
impl<T: Endian> PartialOrd<Utf16Str<T>> for str {
    fn partial_cmp(&self, other: &Utf16Str<T>) -> Option<Ordering> {
        other.partial_cmp(self).map(Ordering::reverse)
    }
}
impl<T: Endian> PartialOrd<Utf16Str<T>> for &str {
    fn partial_cmp(&self, other: &Utf16Str<T>) -> Option<Ordering> {
        other.partial_cmp(*self).map(Ordering::reverse)
    }
}
impl<T: Endian> PartialOrd<Utf16Str<T>> for dyn AsRef<str> {
    fn partial_cmp(&self, other: &Utf16Str<T>) -> Option<Ordering> {
        self.as_ref().partial_cmp(other).map(Ordering::reverse)
    }
}

pub mod iter {
    use crate::endian::Endian;

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
        /// Creates a new iterator over the characters of the given UTF-16 string.
        #[must_use]
        pub fn new<T: Endian>(str: &'a super::Utf16Str<T>) -> Self {
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
    use super::{Endian, Utf16Str};

    pub mod starts_with {
        use super::{Endian, Utf16Str};

        pub trait PrefixChecker {
            /// Returns true if `T` starts with `self`.
            /// Doesn't do any character normalization.
            fn is_prefix_of<T: Endian>(&self, against: &Utf16Str<T>) -> bool;
        }
    
        impl PrefixChecker for str {
            fn is_prefix_of<T: Endian>(&self, against: &Utf16Str<T>) -> bool {
                <dyn AsRef<Self> as PrefixChecker>::is_prefix_of(&self, against)
            }
        }

        impl PrefixChecker for &str {
            fn is_prefix_of<T: Endian>(&self, against: &Utf16Str<T>) -> bool {
                <dyn AsRef<str> as PrefixChecker>::is_prefix_of(&self, against)
            }
        }

        impl PrefixChecker for dyn AsRef<str> + '_ {
            fn is_prefix_of<T: Endian>(&self, against: &Utf16Str<T>) -> bool {
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


        impl<T: Endian> PrefixChecker for Utf16Str<T> {
            fn is_prefix_of<U: Endian>(&self, against: &Utf16Str<U>) -> bool {
                self.bytes().starts_with(against.bytes())
            }
        }
    }
}


/// Compile-time UTF-16 conversion functions, primarily for internal use in the [`utf16!`] and [`utf16_literal!`] macro.
pub mod convert {
    use crate::endian::Endian;

    #[must_use]
    pub const fn predict_equivalent_byte_size(str: &str) -> usize {
        let mut size = 0;
        let mut index = 0;
        let bytes = str.as_bytes();
    
        while index < bytes.len() {
            match bytes[index] {
                0x00..=0x7F => { // 1-byte UTF-8 -> 2 bytes UTF-16
                    size += 2;
                    index += 1;
                }
                0xC0..=0xDF => { // 2-byte UTF-8 -> 2 bytes UTF-16
                    size += 2;
                    index += 2;
                }
                0xE0..=0xEF => { // 3-byte UTF-8 -> 2 bytes UTF-16
                    size += 2;
                    index += 3;
                }
                0xF0..=0xF7 => { // 4-byte UTF-8 -> 4 bytes UTF-16 (surrogate pair)
                    size += 4;
                    index += 4;
                }
                _ => { index += 1; } // invalid; skip
            }
        }
    
        size
    }
    
    /// # Safety
    /// - The `onto` slice must be at least as large as the value returned by [`predict_equivalent_byte_size`] called with the `literal` string.
    /// 
    /// # Alternatives
    /// - [`Utf16Str::from_str`](super::Utf16Str::new): creates a UTF-16 string from a string at runtime
    /// - [`encode_literal`]: allocates the output buffer and returns it, as opposed to requiring it to be passed in.
    pub const unsafe fn encode_literal_in<E: Endian>(literal: &str, onto: &mut [u8]) {
        let original = literal.as_bytes();

        let mut idx_in = 0;
        let mut idx_out = 0;

        macro_rules! split {
            ($value: expr) => {
                if E::IS_LITTLE {
                    $value.to_le_bytes()
                } else {
                    $value.to_be_bytes()
                }
            };
        }

        while idx_in < original.len() {
            match original[idx_in] {
                byte @ 0x00..=0x7F => { // 1-byte UTF-8
                    let code_unit = byte as u16;
                    let [low, high] = split!(code_unit);
                    onto[idx_out] = low;
                    onto[idx_out + 1] = high;
                    
                    idx_in += 1;
                    idx_out += 2;
                }
                byte @ 0xC0..=0xDF => { // 2-byte UTF-8
                    let byte2 = original[idx_in + 1];
                    let code_unit = (((byte & 0x1F) as u16) << 6) | ((byte2 & 0x3F) as u16);
                    let [low, high] = split!(code_unit);
                    
                    onto[idx_out] = low;
                    onto[idx_out + 1] = high;
                    
                    idx_in += 2;
                    idx_out += 2;
                }
                byte @ 0xE0..=0xEF => { // 3-byte UTF-8
                    let byte2 = original[idx_in + 1];
                    let byte3 = original[idx_in + 2];
                    let code_unit = (((byte & 0x0F) as u16) << 12)
                        | (((byte2 & 0x3F) as u16) << 6)
                        | ((byte3 & 0x3F) as u16);
                    let [low, high] = split!(code_unit);
                    
                    onto[idx_out] = low;
                    onto[idx_out + 1] = high;
                    
                    idx_in += 3;
                    idx_out += 2;
                }
                byte @ 0xF0..=0xF7 => { // 4-byte UTF-8
                    let byte2 = original[idx_in + 1];
                    let byte3 = original[idx_in + 2];
                    let byte4 = original[idx_in + 3];
                    let code_point = (((byte & 0x07) as u32) << 18)
                        | (((byte2 & 0x3F) as u32) << 12)
                        | (((byte3 & 0x3F) as u32) << 6)
                        | ((byte4 & 0x3F) as u32);
                        
                    #[allow(clippy::cast_possible_truncation)]
                    let high_surrogate = (((code_point - 0x10000) >> 10) as u16) + 0xD800;
                    let low_surrogate = (((code_point - 0x10000) & 0x3FF) as u16) + 0xDC00;
                    let [high_low, high_high] = split!(high_surrogate);
                    let [low_low, low_high] = split!(low_surrogate);
                    
                    onto[idx_out] = high_low;
                    onto[idx_out + 1] = high_high;
                    onto[idx_out + 2] = low_low;
                    onto[idx_out + 3] = low_high;
                    
                    idx_in += 4;
                    idx_out += 4;
                },
                _ => { idx_in += 1; } // invalid; skip
            }
        }
    }

    /// # Safety
    /// - The `SIZE` constant parameter **MUST** be equal to the output of [`predict_equivalent_byte_size`] called with the `literal` string.
    /// 
    /// # Alternatives
    /// - [`Utf16Str::from_str`](super::Utf16Str::new): creates a UTF-16 string from a string at runtime
    /// - [`encode_literal_in`]: does not reserve an output buffer, requires it to be passed in.
    #[must_use]
    pub const unsafe fn encode_literal<E: Endian, const SIZE: usize>(literal: &str) -> [u8; SIZE] {
        let mut buffer = [0u8; SIZE];
        unsafe { encode_literal_in::<E>(literal, &mut buffer); }
        buffer
    }
}

/// Creates a UTF-16 string from a string and specified endianness at compile time.
/// 
/// If a `[heap]` prefix is given before the inclusion of the, the string is allocated on the heap at runtime backed by a vector (requires the `alloc` feature).
#[macro_export]
macro_rules! utf16 {
    ($e: ident, $v: literal) => {
        const {
            const SIZE: usize = $crate::utf16::convert::predict_equivalent_byte_size($v);
            const VALUE: [u8; SIZE] = unsafe { $crate::utf16::convert::encode_literal::<$crate::endian::aliases::$e, SIZE>($v) };
            unsafe { $crate::utf16::Utf16Str::<$crate::endian::aliases::$e>::new_unchecked(&VALUE) }
        }
    };
    ([heap] $e: ident $v: literal) => {
        #[cfg(feature = "alloc")]
        {
            const SIZE: usize = $crate::utf16::convert::predict_equivalent_byte_size($v);
            let mut vec = alloc::vec::Vec::with_capacity(SIZE);
            unsafe { $crate::utf16::convert::encode_literal_in($v, &mut vec) };
            $crate::utf16::Utf16Str::<$crate::endian::resolvers::$e>::new_unchecked(&vec)
        }
        #[cfg(not(feature = "alloc"))]
        const { panic!("utf16! macro usage with [heap] prefix requires the alloc feature to build a vector") }
    };
}

pub use utf16;

#[cfg(test)]
mod tests {
    #[expect(dead_code)]
    const CONSTANT_MACRO_USAGE_TEST: &super::Utf16Str<crate::endian::LittleEndian> = utf16!(LE, "hello, world!");

    #[test]
    fn predict_byte_size() {
        use super::convert::predict_equivalent_byte_size;
        for str in ["", "hello", "こんにちは", "👨‍👩‍👧‍👦", "a𐍈b𐍈c", "🧑🏾‍❤️‍💋‍🧑🏻",] {
            let predicted = predict_equivalent_byte_size(str);
            let actual = str.encode_utf16().map(|_| 2).sum();
            assert_eq!(predicted, actual, "Failed on string: {str}");
        }
    }

    #[test]
    fn macro_basic_endianness() {
        assert_eq!(utf16!(LE, "hello").bytes(), b"h\0e\0l\0l\0o\0");
        assert_eq!(utf16!(BE, "hello").bytes(), b"\0h\0e\0l\0l\0o");
    }

    #[test]
    fn display() {
        extern crate alloc;
        use alloc::string::ToString;
        for str in ["", "hello", "こんにちは", "👨‍👩‍👧‍👦", "a𐍈b𐍈c", "🧑🏾‍❤️‍💋‍🧑🏻",] {
            let le = str.encode_utf16().flat_map(u16::to_le_bytes).collect::<alloc::vec::Vec<u8>>();
            let be = str.encode_utf16().flat_map(u16::to_be_bytes).collect::<alloc::vec::Vec<u8>>();
            let le = super::Utf16Str::<crate::endian::LittleEndian>::new(&le[..]).unwrap().to_string();
            let be = super::Utf16Str::<crate::endian::   BigEndian>::new(&be[..]).unwrap().to_string();
            assert_eq!(str, le);
            assert_eq!(str, be);
        }
    }

    mod equality {
        #[test]
        fn with_str() {
            assert!(utf16!(sys, "jor") == "jor");
            assert!(utf16!(sys, "👨‍👩‍👧‍👦") == "👨‍👩‍👧‍👦");
            assert!(utf16!(sys, "🙃") == "🙃");
            assert!(utf16!(sys, "🧑🏾‍❤️‍💋‍🧑🏻") == "🧑🏾‍❤️‍💋‍🧑🏻");
        }

        #[test]
        fn cross_endian() {
            assert!(utf16!(LE, "jor") == utf16!(BE, "jor"));
            assert!(utf16!(LE, "👨‍👩‍👧‍👦")  == utf16!(BE, "👨‍👩‍👧‍👦"));
            assert!(utf16!(LE, "🙃")  == utf16!(BE, "🙃"));
            assert!(utf16!(LE, "🧑🏾‍❤️‍💋‍🧑🏻")  == utf16!(BE, "🧑🏾‍❤️‍💋‍🧑🏻"));
        }
    }


    #[test]
    fn cmp() {
        macro_rules! test_equiv {
            ($(($lhs: literal, $rhs: literal) $(,)?)*) => {
                $(
                    assert_eq!(utf16!(sys, $lhs).partial_cmp(utf16!(sys, $rhs)), $lhs.partial_cmp($rhs));
                    assert_eq!(utf16!(sys, $lhs).partial_cmp($rhs), $lhs.partial_cmp(utf16!(sys, $rhs)));
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
