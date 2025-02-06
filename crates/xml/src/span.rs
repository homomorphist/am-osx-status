use core::num::NonZeroUsize;

/// NOTE: Equality is performed by content.
#[derive(Clone, Copy)]
pub struct Span<'a> {
    pub(crate) top: *const u8, // top-level sliced sting start addr
    pub(crate) offset: usize,
    pub(crate) length: usize,
    pub(crate) lifetime: core::marker::PhantomData<&'a str>, // top level
}
impl<'a> Span<'a> {
    pub const fn new_root(str: &'a str) -> Self {
        Self {
            top: str.as_ptr(),
            offset: 0,
            length: str.len(),
            lifetime: core::marker::PhantomData
        }
    }
    
    pub fn start_location(&self) -> SingleFileLocation {
        SingleFileLocation::from(self)
    }

    /// View the string content of the span.
    pub const fn as_str(&self) -> &'a str {
        let start = unsafe { self.top.add(self.offset) };
        let slice = unsafe { core::slice::from_raw_parts(start, self.length) };
        unsafe { core::str::from_utf8_unchecked(slice) }
    }
    
    /// # Safety
    /// - Must align on valid chars so the output is UTF-8.
    /// 
    /// # Panics
    /// - If the start occurs before the end.
    /// - If the slice extends extends out of bounds.
    pub const unsafe fn slice_bytes_inclusive(&self, start: usize, end: Option<NonZeroUsize>) -> Span<'a> {
        let offset = self.offset + start;
        let end = if let Some(end) = end { end.get() } else { self.length };
        let length = end.checked_sub(start).expect("end > start");
        assert!(offset + length <= self.offset + self.length, "cannot slice out of bounds");
        Self {
            top: self.top,
            length,
            offset,
            lifetime: core::marker::PhantomData,
        }
    }

    /// # Safety
    /// - Must align on valid chars so the output is UTF-8.
    /// - Must not exceed the byte range of the top-most string.
    /// 
    /// # Panics
    /// - If the start occurs before the end.
    pub const unsafe fn slice_bytes_inclusive_allow_oob(&self, start: usize, end: Option<NonZeroUsize>) -> Span<'a> {
        let offset = self.offset + start;
        let end = if let Some(end) = end { end.get() } else { self.length };
        let length = end.checked_sub(start).expect("end > start");
        Self {
            top: self.top,
            length,
            offset,
            lifetime: core::marker::PhantomData,
        }
    }

    /// # Safety
    /// - Must align on valid chars so the output is UTF-8.
    /// 
    /// # Panics
    /// - If the slice extends extends out of bounds.
    pub const unsafe fn slice_bytes_off_of_end_inclusive(&self, amount: usize) -> Span<'a> {
        self.slice_bytes_inclusive(self.length - amount, None)
    }
}
impl core::ops::Deref for Span<'_> {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}
impl core::fmt::Display for Span<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}
impl Eq for Span<'_> {}
impl PartialEq for Span<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}
impl PartialEq<str> for Span<'_> {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}
impl PartialEq<&str> for Span<'_> {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}
impl core::fmt::Debug for Span<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Span({:?}@{}[{}])={:?}", self.top, self.offset, self.length, self.as_str())
    }
}
impl core::hash::Hash for Span<'_> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        state.write(self.as_str().as_bytes())
    }
}

/// The location of a character in a document, zero-indexed.
/// The [`core::fmt::Display`] implementation prints it out as one-indexed.
pub struct SingleFileLocation {
    pub line: u32,
    pub column: u32,
}
impl<'a> From<&'a Span<'_>> for SingleFileLocation {
    fn from(span: &'a Span) -> Self {
        let mut row = 0;
        let mut col = 0;
        let before = unsafe { core::slice::from_raw_parts(span.top, span.offset) };
        let before  = unsafe { core::str::from_utf8_unchecked(before) };
        for char in before.chars() {
            if char == '\n' {
                col = 0;
                row += 1;
            } else {
                col += 1;
            }
        };
        Self {
            line: row,
            column: col
        }
    }
}
impl core::fmt::Display for SingleFileLocation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "line {} column {}", self.line + 1, self.column + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod span {
        use super::*;

        #[test]
        fn creation() {
            static STR: &str = "jor";
            let span = Span::new_root(STR);
            assert_eq!(span.top, STR.as_ptr(), "points to original string");
            assert_eq!(span.length, STR.len(), "has correct length");
            assert_eq!(span.offset, 0, "contains no initial offset");
        }
    
        macro_rules! test_slice {
            (#CHK: $span: expr, $slice: expr, $offset: literal) => {
                assert_eq!($span.length, $slice.len(), "has correctly adjusted length");
                assert_eq!($span.offset, $offset, "has correctly adjusted offset");
                assert_eq!(format!("{}", $span), $slice, "correctly formats the slice of the string")
            };

            (#RESOLVE_END: None) => { None };
            (#RESOLVE_END: $val: literal) => { Some(core::num::NonZero::new($val).unwrap()) };

            (@$span: ident, [ $start: literal, $end: tt ], expect: { $slice: literal; offset: $expected_offset: literal }) => {
                {
                    let sliced = unsafe { $span.slice_bytes_inclusive($start, test_slice!(#RESOLVE_END: $end)) };
                    test_slice!(#CHK: sliced, $slice, $expected_offset);
                    sliced
                }
            };

            ($text: literal, [ $start: literal, $end: tt ], expect: { $slice: literal; offset: $expected_offset: literal }) => {
                {
                    let span = Span::new_root($text);
                    test_slice!(@span, [$start, $end], expect: { $slice; offset: $expected_offset })
                }
            };
        }

        #[test]
        fn slicing() {
            test_slice!("123 abc xyz", [4, 7], expect: { "abc"; offset: 4 });
            test_slice!("123 abc xyz", [0, 3], expect: { "123"; offset: 0 });
            let skip_one = test_slice!("123 abc xyz", [4, None], expect: { "abc xyz"; offset: 4 });
            test_slice!(@skip_one, [0, 3],    expect: { "abc"; offset: 4 });
            test_slice!(@skip_one, [4, None], expect: { "xyz"; offset: 8 });

            // TODO: Test invalid slices.
        }
    }
}

