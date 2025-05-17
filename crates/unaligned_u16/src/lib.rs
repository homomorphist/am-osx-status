#[cfg(feature = "utf16")]
pub mod utf16;

pub mod error {
    /// An error indicating that the byte-length of the slice was not a multiple of two.
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
}

pub mod iter {
    use super::UnalignedU16Slice;

    #[repr(transparent)]
    pub struct UnalignedU16SliceIterator<'a>(&'a UnalignedU16Slice);
    impl<'a> UnalignedU16SliceIterator<'a> {
        pub fn new(slice: &'a super::UnalignedU16Slice) -> Self {
            Self(slice)
        }
        pub fn remaining(&self) -> usize {
            self.0.len()
        }
    }
    impl Iterator for UnalignedU16SliceIterator<'_> {
        type Item = u16;
        fn next(&mut self) -> Option<Self::Item> {
            if self.0.is_empty() { return None }
            let u16 = self.0.get(0).unwrap();
            self.0 = unsafe { UnalignedU16Slice::new_unchecked(&self.0.bytes()[2..]) };
            Some(u16)
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            let rem = self.remaining();
            (rem, Some(rem))
        }
    }
    impl ExactSizeIterator for UnalignedU16SliceIterator<'_> {
        fn len(&self) -> usize {
            self.0.len()
        }
    }
    impl core::iter::FusedIterator for UnalignedU16SliceIterator<'_> {}
    impl DoubleEndedIterator for UnalignedU16SliceIterator<'_> {
        fn next_back(&mut self) -> Option<Self::Item> {
            if self.0.is_empty() { return None }
            let len = self.0.len();
            let u16 = self.0.get(len - 1).unwrap();
            self.0 = &self.0[..len - 1];
            Some(u16)
        }
    }
}

// pub(crate)
pub fn u16_slice_as_u8_slice(slice: &[u16]) -> &[u8] {
    let len = slice.len() * 2;
    let ptr = slice.as_ptr().cast();
    unsafe { core::slice::from_raw_parts(ptr, len) }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct UnalignedU16Slice([u8]);
impl<'a> UnalignedU16Slice {
    pub fn new(slice: &[u8]) -> Result<&Self, error::BadByteLength> {
        if slice.len() % 2 != 0 { return Err(error::BadByteLength) }
        Ok(unsafe { Self::new_unchecked(slice) })
    }

    /// # Safety
    /// - The provided slice must have a length that is a multiple of two.
    pub unsafe fn new_unchecked(slice: &[u8]) -> &Self {
        unsafe { core::mem::transmute(slice) }
    }

    /// Returns the amount of `u16` elements.
    pub fn len(&self) -> usize {
        self.bytes().len() / 2
    }

    /// Returns true if the slice is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn byte_len(&self) -> usize {
        self.bytes().len()
    }

    pub fn bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn get(&self, index: usize) -> Option<u16> {
        if index >= self.len() { return None }
        Some(unsafe { self.get_unchecked(index) })
    }

    pub unsafe fn get_unchecked(&self, index: usize) -> u16 {
        let real = index * 2;
        let u8 = self.bytes();
        ((*u8.get_unchecked(real + 1) as u16) << 8) |
          *u8.get_unchecked(real)     as u16
    }

    pub fn iter(&'a self) -> iter::UnalignedU16SliceIterator<'a> {
        self.into_iter()
    }
}
// impl core::ops::Deref for UnalignedU16Slice {
//     type Target = [u8];
//     fn deref(&self) -> &Self::Target {
//         &self.0
//     }
// }
impl<'a> TryFrom<&'a [u8]> for &'a UnalignedU16Slice {
    type Error = error::BadByteLength;
    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        UnalignedU16Slice::new(value)
    }
}
impl<'a> From<&'a [u16]> for &'a UnalignedU16Slice {
    fn from(value: &'a [u16]) -> Self {
        let bytes = u16_slice_as_u8_slice(value);
        unsafe { UnalignedU16Slice::new_unchecked(bytes) }
    }
}
impl<'a> From<&'a UnalignedU16Slice> for &'a [u8] {
    fn from(value: &'a UnalignedU16Slice) -> Self {
        &value.0
    }
}
impl<'a> TryFrom<&'a UnalignedU16Slice> for &'a [u16] {
    type Error = error::AlignmentError;
    fn try_from(value: &'a UnalignedU16Slice) -> Result<Self, Self::Error> {
        let (unaligned, aligned, _) = unsafe { value.0.align_to::<u16>() };
        if unaligned.is_empty() { Ok(aligned) } else { Err(error::AlignmentError) }
    }
}
impl<'a> IntoIterator for &'a UnalignedU16Slice {
    type Item = u16;
    type IntoIter = iter::UnalignedU16SliceIterator<'a>;
    fn into_iter(self) -> Self::IntoIter {
        iter::UnalignedU16SliceIterator::new(self)
    }
}

impl core::ops::Index<core::ops::Range<usize>> for UnalignedU16Slice {
    type Output = UnalignedU16Slice;
    fn index(&self, index: core::ops::Range<usize>) -> &Self::Output {
        let len: usize = self.len();
        let s = index.start;
        let e = index.end;
        if s > len || e > len { panic!("index out of bounds") }
        let slice = &self.0[(s * 2)..(e * 2)];
        unsafe { UnalignedU16Slice::new_unchecked(slice) }
    }
}
impl core::ops::Index<core::ops::RangeFrom<usize>> for UnalignedU16Slice {
    type Output = UnalignedU16Slice;
    fn index(&self, index: core::ops::RangeFrom<usize>) -> &Self::Output {
        let len: usize = self.len();
        if index.start >= len { panic!("index out of bounds") }
        let slice = &self.0[(index.start * 2)..];
        unsafe { UnalignedU16Slice::new_unchecked(slice) }
    }
}
impl core::ops::Index<core::ops::RangeInclusive<usize>> for UnalignedU16Slice {
    type Output = UnalignedU16Slice;
    fn index(&self, index: core::ops::RangeInclusive<usize>) -> &Self::Output {
        let len: usize = self.len();
        let s = *index.start();
        let e = *index.end();
        if s > len || e > len { panic!("index out of bounds") }
        let slice = &self.0[(s * 2)..=((e * 2) + 1)];
        unsafe { UnalignedU16Slice::new_unchecked(slice) }
    }
}
impl core::ops::Index<core::ops::RangeTo<usize>> for UnalignedU16Slice {
    type Output = UnalignedU16Slice;
    fn index(&self, index: core::ops::RangeTo<usize>) -> &Self::Output {
        let len: usize = self.len();
        if index.end > len { panic!("index out of bounds") }
        let slice = &self.0[..(index.end * 2)];
        unsafe { UnalignedU16Slice::new_unchecked(slice) }
    }
}
impl core::ops::Index<core::ops::RangeToInclusive<usize>> for UnalignedU16Slice {
    type Output = UnalignedU16Slice;
    fn index(&self, index: core::ops::RangeToInclusive<usize>) -> &Self::Output {
        let len: usize = self.len();
        if index.end > len { panic!("index out of bounds") }
        let slice = &self.0[..(index.end + 1) * 2];
        unsafe { UnalignedU16Slice::new_unchecked(slice) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iter() {
        let slice = [0x01, 0x02, 0x03, 0x04];
        let unaligned = UnalignedU16Slice::new(&slice).unwrap();
        let mut iter = unaligned.iter();
        assert_eq!(iter.len(), 2);
        assert_eq!(iter.next(), Some(0x0201));
        assert_eq!(iter.next(), Some(0x0403));
        assert_eq!(iter.len(), 0);
        assert_eq!(iter.next(), None);

        let mut iter = unaligned.iter();
        assert_eq!(iter.next_back(), Some(0x0403));
        assert_eq!(iter.next_back(), Some(0x0201));
        assert_eq!(iter.next_back(), None);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn get() {
        let slice = [0x01, 0x02, 0x03, 0x04];
        let unaligned = UnalignedU16Slice::new(&slice).unwrap();
        assert_eq!(unaligned.get(0), Some(0x0201));
        assert_eq!(unaligned.get(1), Some(0x0403));
        assert_eq!(unaligned.get(2), None);
    }

    #[test]
    fn ranged_indexing() {
        let slice = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let unaligned = UnalignedU16Slice::new(&slice).unwrap();
        assert_eq!(unaligned[0.. 0], *UnalignedU16Slice::new(&slice[0.. 0]).unwrap());
        assert_eq!(unaligned[0.. 1], *UnalignedU16Slice::new(&slice[0..=1]).unwrap());
        assert_eq!(unaligned[1..=2], *UnalignedU16Slice::new(&slice[2..=5]).unwrap());
        assert_eq!(unaligned[1..],   *UnalignedU16Slice::new(&slice[2..]).unwrap());
        assert_eq!(unaligned[.. 2],  *UnalignedU16Slice::new(&slice[..=3]).unwrap());
        assert_eq!(unaligned[..=2],  *UnalignedU16Slice::new(&slice[..=5]).unwrap());
    }
}
