use core::num::NonZeroUsize;

// We use a NonZeroUsize since if constructed safely,
// the root location will always be a valid string thus the first index is always okay.
#[derive(thiserror::Error, Debug)]
pub enum InvalidCharacterBoundary {
    #[error("invalid character boundary at start index {0}")]
    Start(NonZeroUsize),
    #[error("invalid character boundary at end index {0}")]
    End(NonZeroUsize),
}



#[derive(thiserror::Error, Debug)]
pub enum SpanSliceError {
    #[error("{0}")]
    InvalidCharBoundary(InvalidCharacterBoundary),
    #[error("slice out of bounds by {by} bytes")]
    OutOfBounds { by: NonZeroUsize },
    #[error("ending index is non-representable (exceeded max usize)")]
    EndIndexOverflow,
}

/// An immutable string slice with a reference to the top-level string it was sliced from to compute offsets.
/// 
/// Equality is performed by value, not address.
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

    pub const fn new_unchecked(top: *const u8, offset: usize, length: usize) -> Self {
        Self {
            top,
            offset,
            length,
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

    const fn test_character_boundaries(&self, start: usize, end: usize) -> Option<InvalidCharacterBoundary> {
        let str = self.as_str();
        const BAD_BOUNDARY_AT_ZERO: &str = "source string had bad char boundary at index 0; violates valid construction invariant";
        if !str.is_char_boundary(start) { return Some(InvalidCharacterBoundary::Start(NonZeroUsize::new(start).expect(BAD_BOUNDARY_AT_ZERO))); }
        if !str.is_char_boundary(end)   { return Some(InvalidCharacterBoundary::End  (NonZeroUsize::new(end)  .expect(BAD_BOUNDARY_AT_ZERO))); }
        None
    }

    pub const fn try_slice(&self, start: usize, length: usize) -> Result<Span<'a>, SpanSliceError> {
        let str = self.as_str();
        let Some(end) = start.checked_add(length) else { return Err(SpanSliceError::EndIndexOverflow); };
        if let Some(bad_boundary) = self.test_character_boundaries(start, end) { return Err(SpanSliceError::InvalidCharBoundary(bad_boundary)); }
        let offset = self.offset + start;
        if offset + length > self.offset + self.length { return Err(SpanSliceError::OutOfBounds { by: NonZeroUsize::new(offset + length - (self.offset + self.length)).unwrap() }); }
        Ok(Self { top: self.top, length, offset, lifetime: core::marker::PhantomData, })
    }
    
    pub fn slice(&self, start: usize, length: usize) -> Span<'a> {
        self.try_slice(start, length).expect("invalid slice")
    }

    pub fn slice_with<S: SpanSlicer>(&self, slicer: S) -> Span<'a> {
        slicer.slice_span(self)
    }

    /// Alias for `slice_with`.
    pub fn range(&self, range: impl SpanSlicer) -> Span<'a> {
        self.slice_with(range)
    }

    /// # Safety
    /// - Must be called with valid character boundaries.
    /// - Cannot exceed the bounds of the calling span.
    pub unsafe fn slice_unchecked(&self, start: usize, length: usize) -> Span<'a> {
        let offset = self.offset + start;
        Self { top: self.top, length, offset, lifetime: core::marker::PhantomData, }
    }
 
    /// Slice with a signed start index, allowing for backwards slicing.
    /// If the start index would underflow, it will be clamped to zero.
    /// If the length would exceed the bounds of the span, it will clamp to the maximum possible length.
    pub fn slice_signed_clamping(&self, start: isize, length: usize) -> Span<'a> {
        let offset = self.offset.checked_add_signed(start).unwrap_or(0);
        let length = core::cmp::min(length, self.offset + self.length - offset);
        Self { top: self.top, length, offset, lifetime: core::marker::PhantomData }
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

pub trait SpanSlicer {
    fn slice_span<'a>(&self, span: &Span<'a>) -> Span<'a>;
}
impl SpanSlicer for core::ops::Range<usize> {
    fn slice_span<'a>(&self, span: &Span<'a>) -> Span<'a> {
        span.slice(self.start, self.end - self.start)
    }
}
impl SpanSlicer for core::ops::RangeFrom<usize> {
    fn slice_span<'a>(&self, span: &Span<'a>) -> Span<'a> {
        span.slice(self.start, span.length - self.start)
    }
}
impl SpanSlicer for core::ops::RangeTo<usize> {
    fn slice_span<'a>(&self, span: &Span<'a>) -> Span<'a> {
        span.slice(0, self.end)
    }
}
impl SpanSlicer for core::ops::RangeFull {
    fn slice_span<'a>(&self, span: &Span<'a>) -> Span<'a> {
        *span
    }
}
impl SpanSlicer for core::ops::RangeInclusive<usize> {
    fn slice_span<'a>(&self, span: &Span<'a>) -> Span<'a> {
        span.slice(*self.start(), self.end() - self.start() + 1)
    }
}
impl SpanSlicer for core::ops::RangeToInclusive<usize> {
    fn slice_span<'a>(&self, span: &Span<'a>) -> Span<'a> {
        span.slice(0, self.end + 1)
    }
}

/// The location of a character in a document, zero-indexed.
/// The [`core::fmt::Display`] implementation prints it out as one-indexed.
pub struct SingleFileLocation {
    pub line: u32,
    pub column: u32,
}
impl<'a> From<&'a Span<'a>> for SingleFileLocation {
    fn from(span: &'a Span) -> Self {
        let mut row = 0;
        let mut col = 0;
        let before = span.as_str();
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

            (@$span: ident, $range: expr, expect: { $slice: literal; offset: $expected_offset: literal }) => {
                {
                    let sliced = unsafe { $span.slice_with($range) };
                    test_slice!(#CHK: sliced, $slice, $expected_offset);
                    sliced
                }
            };

            ($text: literal, $range: expr, expect: { $slice: literal; offset: $expected_offset: literal }) => {
                {
                    let span = Span::new_root($text);
                    test_slice!(@span, $range, expect: { $slice; offset: $expected_offset })
                }
            };
        }

        #[test]
        fn slicing() {
            test_slice!("123 abc xyz", (4..7), expect: { "abc"; offset: 4 });
            test_slice!("123 abc xyz", (0..3), expect: { "123"; offset: 0 });
            let skip_one = test_slice!("123 abc xyz", (4..), expect: { "abc xyz"; offset: 4 });
            test_slice!(@skip_one, (0..3), expect: { "abc"; offset: 4 });
            test_slice!(@skip_one, (4..),  expect: { "xyz"; offset: 8 });

            // TODO: Test invalid slices.
        }
    }
}

