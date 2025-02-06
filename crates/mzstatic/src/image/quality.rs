#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// A number between 0 and 999, inclusive on both ends, representing the quality of an image (i.e. it inversely correlates with compression level).
pub struct Quality(u16);
impl Quality {
    /// The maximum quality level, representing a very compressed and likely low-quality image.
    pub const MIN_INCLUSIVE: u16 = 0;
    /// The maximum quality level, representing a minimally compressed and likely high-quality image.
    pub const MAX_INCLUSIVE: u16 = 999;

    /// Returns whether the given value would be a valid quality.
    pub fn test(value: &u16) -> bool {
        (Self::MIN_INCLUSIVE..=Self::MAX_INCLUSIVE).contains(value)
    }

    /// Returns the inner stored value.
    pub fn into_inner(self) -> u16 {
        self.0
    }

    /// Returns the inner stored value.
    pub fn get(&self) -> &u16 {
        &self.0
    }

    /// Returns a new quality, depending on whether the given input value was in range.
    pub fn new(value: u16) -> Result<Self, OutOfRangeError> {
        if !Self::test(&value) { return Err(OutOfRangeError) }
        Ok(unsafe { Self::new_unchecked(value) })
    }

    /// Returns a new quality, trusting that the given value is known to be in the valid range.
    /// 
    /// # Safety
    /// - The provided number must be between 0 and 999, inclusive on both ends.
    pub unsafe fn new_unchecked(value: u16) -> Self {
        Self(value)
    }
}
impl core::fmt::Display for Quality {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.get())
    }
}
impl TryFrom<u16> for Quality {
    type Error = OutOfRangeError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// An error that occurs when the given quality is out of range.
pub struct OutOfRangeError;
impl core::error::Error for OutOfRangeError {}
impl core::fmt::Display for OutOfRangeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "quality out of bounds: must satisfy range [{}, {}]",
            Quality::MIN_INCLUSIVE,
            Quality::MAX_INCLUSIVE
        )
    }
}
