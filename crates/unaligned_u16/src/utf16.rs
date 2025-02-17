use crate::UnalignedU16Slice;

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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Utf16Str<'a> { pub bytes: UnalignedU16Slice<'a> }
impl<'a> Utf16Str<'a> {
    pub fn new(slice: impl TryInto<UnalignedU16Slice<'a>, Error = crate::error::BadByteLength>) -> Result<Self, error::InvalidUtf16> {
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
    
    pub fn utf8_byte_len(&self) -> usize {
        self.chars().map(|char| char.len_utf8()).sum()
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
            string.push(char);
        }
        string
    }
}

#[cfg(test)]
mod test {
    use crate::u16_slice_as_u8_slice;
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
