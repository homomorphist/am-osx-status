#[cfg(feature = "utf16")]
pub mod utf16;
pub mod endian;

use endian::Endianness;

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

    type EndianMarkedU16SliceAddress<'a> = ointers::NotNull<u8, 0, true, 0>; 

    pub struct UnalignedU16SliceIterator<'a> {
        ptr: EndianMarkedU16SliceAddress<'a>,
        len: usize,
        _lifetime: core::marker::PhantomData<&'a ()>,
    }
    impl<'a> UnalignedU16SliceIterator<'a> {
        pub fn new(slice: &'a super::UnalignedU16Slice, endianness: super::Endianness) -> Self {
            let ointer: EndianMarkedU16SliceAddress<'a> = unsafe { ointers::NotNull::new({
                let ptr = core::ptr::addr_of!(slice.0) as *mut u8;
                core::ptr::NonNull::new_unchecked(ptr)
            }) };

            let ointer = ointer.steal(match endianness {
                super::Endianness::Little => 0,
                super::Endianness::Big =>    1,
            });

            Self {
                ptr: ointer,
                len: slice.len(),
                _lifetime: core::marker::PhantomData,
            }
        }
        fn as_slice(&self) -> &'a UnalignedU16Slice {
            unsafe {
                UnalignedU16Slice::new_unchecked(
                    core::slice::from_raw_parts(
                        self.ptr.as_non_null().as_ptr(),
                        self.len,
                    )
                )
            }
        }
        fn endianness(&self) -> super::Endianness {
            match self.ptr.stolen() {
                0 => super::Endianness::Little,
                1 => super::Endianness::Big,
                _ => unreachable!(),
            }
        }
        fn shift(&mut self, amount: usize) {
            let stolen = self.ptr.stolen();
            self.ptr = unsafe {
                ointers::NotNull::new_stealing(
                    self.ptr.as_non_null().add(amount),
                    stolen,
                )
            };
        }
        pub fn remaining(&self) -> usize {
            self.len / 2
        }
    }
    impl Iterator for UnalignedU16SliceIterator<'_> {
        type Item = u16;
        fn next(&mut self) -> Option<Self::Item> {
            let slice = self.as_slice();
            if slice.is_empty() { return None }
            let u16 = unsafe { slice.get_unchecked(0, self.endianness()) };
            self.shift(2);
            Some(u16)
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            let rem = self.remaining();
            (rem, Some(rem))
        }
    }
    impl ExactSizeIterator for UnalignedU16SliceIterator<'_> {
        fn len(&self) -> usize {
            self.len / 2
        }
    }
    impl core::iter::FusedIterator for UnalignedU16SliceIterator<'_> {}
    impl DoubleEndedIterator for UnalignedU16SliceIterator<'_> {
        fn next_back(&mut self) -> Option<Self::Item> {
            let slice = self.as_slice();
            if slice.is_empty() { return None }
            let u16 = unsafe { slice.get_unchecked(slice.len() - 1, self.endianness()) };
            self.len -= 2;
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
    pub const fn new(slice: &[u8]) -> Result<&Self, error::BadByteLength> {
        if slice.len() % 2 != 0 { return Err(error::BadByteLength) }
        Ok(unsafe { Self::new_unchecked(slice) })
    }

    /// # Safety
    /// - The provided slice must have a length that is a multiple of two.
    pub const unsafe fn new_unchecked(slice: &[u8]) -> &Self {
        unsafe { core::mem::transmute(slice) }
    }

    /// Returns the amount of `u16` elements.
    pub const fn len(&self) -> usize {
        self.bytes().len() / 2
    }

    /// Returns true if the slice is empty.
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub const fn byte_len(&self) -> usize {
        self.bytes().len()
    }

    pub const fn bytes(&self) -> &[u8] {
        &self.0
    }

    pub const fn get(&self, index: usize, endianness: Endianness) -> Option<u16> {
        if index >= self.len() { return None }
        Some(unsafe { self.get_unchecked(index, endianness) })
    }
    
    /// # Safety
    /// - The index must be less than the length of the slice.
    pub const unsafe fn get_unchecked(&self, index: usize, endianness: Endianness) -> u16 {
        const unsafe fn get_element_const_unchecked<T>(slice: &[T], index: usize) -> u8 {
            let offset = index * core::mem::size_of::<T>();
            let offset = slice.as_ptr().add(offset);
            core::ptr::read(offset as *const u8)
        }

        let real = index * 2;
        let u8 = self.bytes();
        match endianness {
            Endianness::Little => {
                ((get_element_const_unchecked(u8, real + 1) as u16) << 8) |
                  get_element_const_unchecked(u8, real)     as u16
            }
            Endianness::Big => {
                ((get_element_const_unchecked(u8, real) as u16) << 8) |
                  get_element_const_unchecked(u8, real + 1) as u16
            }
        }
    }

    pub fn iter(&'a self, endianness: Endianness) -> iter::UnalignedU16SliceIterator<'a> {
        iter::UnalignedU16SliceIterator::new(self, endianness)
    }
}
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
        let mut iter = unaligned.iter(Endianness::Big);
        assert_eq!(iter.len(), 2);
        assert_eq!(iter.next(), Some(0x0201));
        assert_eq!(iter.next(), Some(0x0403));
        assert_eq!(iter.len(), 0);
        assert_eq!(iter.next(), None);

        let mut iter = unaligned.iter(Endianness::Big);
        assert_eq!(iter.next_back(), Some(0x0403));
        assert_eq!(iter.next_back(), Some(0x0201));
        assert_eq!(iter.next_back(), None);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn get() {
        let slice = [0x01, 0x02, 0x03, 0x04];
        let unaligned = UnalignedU16Slice::new(&slice).unwrap();
        assert_eq!(unaligned.get(0, Endianness::Little), Some(0x0201));
        assert_eq!(unaligned.get(1, Endianness::Little), Some(0x0403));
        assert_eq!(unaligned.get(2, Endianness::Little), None);
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
