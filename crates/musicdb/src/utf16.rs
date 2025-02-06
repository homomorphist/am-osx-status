use std::fmt::Write;

pub mod error {
    /// An error indicating that the byte-length of the slice was not a multiple of tow.
    #[derive(Debug)]
    pub struct BadByteLength;
    impl core::error::Error for BadByteLength {}
    impl core::fmt::Display for BadByteLength {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "byte length must be a multiple of two")
        }
    }

    /// An error indicating that the slice was not correctly aligned.
    #[derive(Debug)]
    pub struct AlignmentError;
    impl core::error::Error for AlignmentError {}
    impl core::fmt::Display for AlignmentError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "slice is not correctly aligned")
        }
    }

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
        BadByteLength(#[from] BadByteLength),
        #[error("{0}")]
        UnpairedSurrogate(#[from] UnpairedSurrogate),
        #[error("{0}")]
        NonCharacterEncountered(#[from] NonCharacterEncountered)
    }
}

pub mod iter {
    pub struct UnalignedU16SliceIterator<'a> { slice: &'a [u8] }
    impl<'a> UnalignedU16SliceIterator<'a> {
        pub fn new(slice: &super::UnalignedU16Slice<'a>) -> Self {
            Self { slice: slice.raw() }
        }
        pub fn remaining(&self) -> usize {
            self.slice.len() / 2
        }
    }
    impl Iterator for UnalignedU16SliceIterator<'_> {
        type Item = u16;
        fn next(&mut self) -> Option<Self::Item> {
            if self.slice.is_empty() { return None }
            let u16 = ((self.slice[1] as u16) << 8) | (self.slice[0] as u16);
            self.slice = &self.slice[2..];
            Some(u16)
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            let rem = self.remaining();
            (rem, Some(rem))
        }
    }
    impl ExactSizeIterator for UnalignedU16SliceIterator<'_> {}
    impl core::iter::FusedIterator for UnalignedU16SliceIterator<'_> {}
    impl DoubleEndedIterator for UnalignedU16SliceIterator<'_> {
        fn next_back(&mut self) -> Option<Self::Item> {
            if self.slice.is_empty() { return None }
            let end = self.slice.len();
            let u16 = ((self.slice[end - 1] as u16) << 8) | (self.slice[end - 2] as u16);
            self.slice = &self.slice[..=end - 3];
            Some(u16)
        }
    }
}

pub(crate) fn u16_slice_as_u8_slice(slice: &[u16]) -> &[u8] {
    let len = slice.len() * 2;
    let ptr = slice.as_ptr().cast();
    unsafe { core::slice::from_raw_parts(ptr, len) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnalignedU16Slice<'a>(&'a [u8]);
impl<'a> UnalignedU16Slice<'a> {
    pub fn new(slice: &'a [u8]) -> Result<Self, error::BadByteLength> {
        if slice.len() % 2 != 0 { return Err(error::BadByteLength) }
        Ok(Self(slice))
    }

    /// # Safety
    /// - The provided slice must have a length that is a multiple of two.
    pub unsafe fn new_unchecked(slice: &'a [u8]) -> Self {
        Self(slice)
    }

    /// Returns the amount of `u16` elements.
    pub fn len(&self) -> usize {
        self.raw().len() / 2
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn byte_len(&self) -> usize {
        self.raw().len()
    }

    pub fn raw(&self) -> &'a [u8] {
        self.0
    }
    pub fn get(&self, index: usize) -> Option<u16> {
        let real = index * 2;
        let u8 = self.raw();
        Some((
            (*u8.get(real + 1)? as u16) << 8) |
             *u8.get(real)?     as u16
        )
    }
    pub fn iter(&self) -> iter::UnalignedU16SliceIterator<'a> {
        self.into_iter()
    }
}
impl core::ops::Deref for UnalignedU16Slice<'_> {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        self.0
    }
}
impl<'a> TryFrom<&'a [u8]> for UnalignedU16Slice<'a> {
    type Error = error::BadByteLength;
    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl<'a> From<&'a [u16]> for UnalignedU16Slice<'a> {
    fn from(value: &'a [u16]) -> Self {
        UnalignedU16Slice(u16_slice_as_u8_slice(value))
    }
}
impl<'a> From<&UnalignedU16Slice<'a>> for &'a [u8] {
    fn from(value: &UnalignedU16Slice<'a>) -> Self {
        value.0
    }
}
impl<'a> TryFrom<&UnalignedU16Slice<'a>> for &'a [u16] {
    type Error = error::AlignmentError;
    fn try_from(value: &UnalignedU16Slice<'a>) -> Result<Self, Self::Error> {
        let (unaligned, aligned, _) = unsafe { value.0.align_to::<u16>() };
        if unaligned.is_empty() { Ok(aligned) } else { Err(error::AlignmentError) }
    }
}
impl<'a> IntoIterator for &UnalignedU16Slice<'a> {
    type Item = u16;
    type IntoIter = iter::UnalignedU16SliceIterator<'a>;
    fn into_iter(self) -> Self::IntoIter {
        iter::UnalignedU16SliceIterator::new(self)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Utf16Str<'a> { pub bytes: UnalignedU16Slice<'a> }
impl<'a> Utf16Str<'a> {
    pub fn new(slice: impl TryInto<UnalignedU16Slice<'a>, Error = error::BadByteLength>) -> Result<Self, error::InvalidUtf16> {
        let bytes = slice.try_into()?;

        // TODO: Actually check that this is valid.

        Ok(Self { bytes })
    }

    /// # Safety
    /// - The provided slice must have a length that is a multiple of two.
    /// - The contents of the slice must be valid UTF-16.
    pub unsafe fn from_bytes_unchecked(slice: &'a [u8]) -> Self {
        Self { bytes: UnalignedU16Slice(slice) }
    }

    pub fn chars(&self) -> impl Iterator<Item = char> + use<'a>  {
        std::char::decode_utf16(self.bytes.iter()).map(|char| {
            char.expect("invalid character encountered despite validation at initialization")
        })
    }
    
    #[allow(clippy::match_overlapping_arm)] // makes it simpler
    pub fn utf8_byte_len(&self) -> usize {
        // TODO: test this better
        let mut sum = 0;
        let mut skip = false;
        let mut hi = false;
        for n in self.bytes.iter() {
            if skip {
                skip = false;
                continue;
            }

            sum += match n {
                (0..=0x007F) => 1,
                (0x0080..=0x07FF) => 2,
                (0x0800..=0xDBFF) => { hi = true; continue }
                (0xDC00..=0xDFFF) => if hi { hi = false; skip = true; 4 } else { unreachable!("low surrogate without high; validity was already checked earlier")  }
                (0x0800..=0xFFFF) => 3,
            }
        }
        sum
    }
}
impl PartialEq<&str> for Utf16Str<'_> {
    fn eq(&self, other: &&str) -> bool {
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
impl PartialOrd<&str> for Utf16Str<'_> {
    fn partial_cmp(&self, other: &&str) -> Option<std::cmp::Ordering> {
        let mut utf8_chars = other.chars();
        let mut utf16_chars = self.chars();
        use core::cmp::Ordering;
        let mut cmp: Option<Ordering> = None;
        loop {
            cmp = match cmp {
                Some(Ordering::Equal) | None => {
                    let lhs = utf16_chars.next();
                    let rhs = utf8_chars.next();
                    if lhs.is_none() && rhs.is_none() { return Some(Ordering::Equal) } else {
                        lhs.partial_cmp(&rhs)
                    }
                },
                ordering => return ordering
            }
        }
    }
}
impl core::fmt::Display for Utf16Str<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use core::fmt::Write;
        for char in self.chars() {
            f.write_char(char)?;
        }
        Ok(())
    }
}
impl core::fmt::Debug for Utf16Str<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if f.sign_minus() {
            f.debug_struct("Utf16Str")
                .field("bytes", &self.bytes)
                .finish()
        } else {
            f.debug_tuple("Utf16Str")
                .field(&self.to_string())
                .finish()
        }
    }
}


impl From<Utf16Str<'_>> for String {
    fn from(val: Utf16Str<'_>) -> Self {
        let mut string = String::with_capacity(val.utf8_byte_len());
        for char in val.chars() {
            core::fmt::Write::write_char(&mut string, char);
        }
        string
    }
}


#[cfg(test)]
mod test {
    use super::*;

    macro_rules! utf16 {
        ($v: literal) => {
            Utf16Str::new(u16_slice_as_u8_slice(&$v.encode_utf16().collect::<Vec<_>>())).unwrap()
        };
    }

    #[test]
    fn eq() {
        assert!(utf16!("jor") == "jor");
        assert!(utf16!("👨‍👩‍👧‍👦") == "👨‍👩‍👧‍👦");
        assert!(utf16!("🙃") == "🙃");
    }
}
