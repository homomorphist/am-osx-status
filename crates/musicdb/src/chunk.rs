use byteorder::{LittleEndian, ReadBytesExt};

/// A chunk is a piece of data distinguished by a four-byte [signature](`Signature`) and a size.
/// 
/// The size is four bytes long, and typically immediately follows the signature, with the
/// exception of the [`Boma`](super::boma::Boma), which has four bytes of padding / unknown data
/// present between the signature and the size.
pub trait Chunk {
    /// The signature for this chunk.
    const SIGNATURE: Signature;

    /// Returns the signature that this chunk uses.
    fn get_signature(&self) -> Signature {
        Self::SIGNATURE
    }
}

pub struct ChunkCursor<'a> {
    data: &'a [u8],
    position: usize,
}
impl<'a> ChunkCursor<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }

    pub fn position(&self) -> usize {
        self.position
    }

    pub fn reading_chunks<'b, T>(&'b mut self, amount: usize) -> ContiguousChunkReader<'b, 'a, T> where T: ReadableChunk<'a> {
        ContiguousChunkReader::new(self, amount)
    }

    pub fn read_signature(&mut self) -> Result<Signature, std::io::Error> {
        let signature = self.peek_signature()?;
        self.advance(Signature::LENGTH as i64)?;
        Ok(signature)
    }

    pub fn read_id<T: crate::id::persistent::Possessor>(&mut self) -> Result<crate::id::persistent::Id<T>, std::io::Error> {
        self.read_u64::<LittleEndian>().map(crate::id::persistent::Id::new)
    }

    pub fn peek_remaining(&self) -> &'a [u8] {
       &self.data[self.position..]
    }

    pub fn peek_slice(&self, amount: usize) -> &'a [u8] {
        &self.peek_remaining()[..amount.min(self.peek_remaining().len())]
    }
    pub fn peek_slice_exact(&self, amount: usize) -> Result<&'a [u8], std::io::Error> {
        match self.peek_remaining().get(..amount) {
            Some(slice) => Ok(slice),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                format!("@ {}: reached EOF after {} / {amount} bytes", self.position, self.peek_remaining().len()),
            ))
        }
    }
    pub fn peek_slice_exact_const<const N: usize>(&self) -> Result<&'a [u8; N], std::io::Error> {
        self.peek_slice_exact(N).map(|slice| unsafe {
            *core::mem::transmute::<
                &&[u8],
                &&[u8; N]
            >(&slice)
        })
    }

    pub fn read_slice_exact(&mut self, amount: usize) -> Result<&'a [u8], std::io::Error> {
        let read = self.peek_slice_exact(amount)?;
        self.position += amount;
        Ok(read)
    }
    pub fn read_slice_exact_const<const N: usize>(&mut self) -> Result<&'a [u8; N], std::io::Error> {
        let read = self.peek_slice_exact_const::<N>()?;
        self.position += N;
        Ok(read)
    }

    /// Returns a slice of up to `amount` bytes, moving forward by the number of bytes read.
    pub fn read_slice(&mut self, amount: usize) -> &'a [u8] {
        let read = self.peek_slice(amount);
        self.position += read.len();
        read
    }

    pub fn read_cstr_exact_with_max_length(&mut self, max_length: usize) -> Result<&'a core::ffi::CStr, std::io::Error> {
        let read = self.read_slice_exact(max_length)?;
        if read.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "no room for cstr to have a null terminator",
            ));
        }
        core::ffi::CStr::from_bytes_until_nul(read).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "did not find null terminator within max length",
            )
        })
    }

    pub fn peek_signature(&mut self) -> Result<Signature, std::io::Error> {
        self.peek_slice_exact_const::<{ Signature::LENGTH }>()
            .map(|bytes| Signature::new(*bytes))
    }

    pub fn backtrack(&mut self, amount: i64) -> Result<u64, std::io::Error> {
        self.advance(-amount)
    }

    pub fn advance(&mut self, amount: i64) -> Result<u64, std::io::Error> {
        <Self as std::io::Seek>::seek(self, std::io::SeekFrom::Current(amount))
    }

    pub fn skip(&mut self, amount: usize) -> Result<u64, std::io::Error> {
        self.advance(amount as i64)
    }

    pub fn set_position(&mut self, position: usize) -> Result<usize, std::io::Error> {
        if position > self.data.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "cannot seek past end",
            ));
        }
        self.position = position;
        Ok(self.position)
    }
}
impl<'a> std::io::Read for ChunkCursor<'a> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        let slice = self.peek_remaining();
        let to_read = buf.len().min(slice.len());
        buf[..to_read].copy_from_slice(&slice[..to_read]);
        self.position += to_read;
        Ok(to_read)
    }
}
impl<'a> std::io::Seek for ChunkCursor<'a> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> Result<u64, std::io::Error> {
        use std::io::SeekFrom;

        match pos {
            SeekFrom::Start(offset) => {
                if offset as usize > self.data.len() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "cannot seek past end",
                    ));
                }
                self.position = offset as usize;
            }
            SeekFrom::End(offset) => {
                let end_pos = self.data.len() as i64 + offset;
                if end_pos < 0 || end_pos as usize > self.data.len() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "cannot seek past end",
                    ));
                }
                self.position = end_pos as usize;
            }
            SeekFrom::Current(offset) => {
                let new_pos = self.position as i64 + offset;
                if new_pos < 0 || new_pos as usize > self.data.len() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "cannot seek past end",
                    ));
                }
                self.position = new_pos as usize;
            }
        }

        Ok(self.position as u64)
    }
}


pub struct ContiguousChunkReader<'a, 'b, T: ReadableChunk<'b>> {
    cursor: &'a mut ChunkCursor<'b>,
    remaining: usize,
    _type: core::marker::PhantomData<T>
}
impl<'a, 'b, T: ReadableChunk<'b>> ContiguousChunkReader<'a, 'b, T> {
    pub fn new(cursor: &'a mut ChunkCursor<'b>, remaining: usize) -> Self {
        Self { cursor, remaining, _type: core::marker::PhantomData }
    }
}
impl<'b, T: ReadableChunk<'b>> Iterator for ContiguousChunkReader<'_, 'b, T> {
    type Item = Result<T, T::ReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let read = T::read(self.cursor);
        self.remaining -= 1;
        Some(read)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl<'b, T: ReadableChunk<'b>> core::iter::FusedIterator for ContiguousChunkReader<'_, 'b, T> {}
impl<'b, T: ReadableChunk<'b>> core::iter::ExactSizeIterator for ContiguousChunkReader<'_, 'b, T> {
    fn len(&self) -> usize {
        self.remaining
    }
}

#[macro_export]
macro_rules! setup_eaters {
    ($cursor: ident, $start_position: ident, $length: ident) => {
        #[allow(unused)] use byteorder::ReadBytesExt as _;
        $crate::chunk::setup_eaters!($cursor, $start_position, $length, ext: false);
    };
    ($cursor: ident, $start_position: ident, $length: ident, ext: false) => {
        #[allow(unused)] macro_rules! skip { ($count: expr) => { $cursor.advance($count) } }
        #[allow(unused)] macro_rules! skip_to_end { () => { $cursor.set_position($start_position as usize + $length as usize) } }
        #[allow(unused)] macro_rules! u64 { () => { $cursor.read_u64::<byteorder::LittleEndian>() } }
        #[allow(unused)] macro_rules! u32 { () => { $cursor.read_u32::<byteorder::LittleEndian>() } }
        #[allow(unused)] macro_rules! u16 { () => { $cursor.read_u16::<byteorder::LittleEndian>() } }
        #[allow(unused)] macro_rules!  u8 { () => { $cursor.read_u8() } }
        #[allow(unused)] macro_rules! cstr { ($size: literal) => {{ $cursor.read_cstr_exact_with_max_length($size) }}}
        #[allow(unused)] macro_rules! id { ($type: ty) => {{ $cursor.read_id::<$type>() }}}
    };
}

pub(crate) use setup_eaters;

/// A four-byte signature, used to identify chunks of data.
/// 
/// All signatures so far have matched the pattern /[a-z]{4}/i.
/// 
/// ## Potential Morphology
/// 
/// Observing the following singular to container patterns:
/// 
/// - `iAma` / `lAma` (artists)
/// - `iama` / `lama` (albums)
/// - `itma` / `ltma` (tracks)
/// - `isma` / `Lsma` (accounts)
/// - `lpma` / `lPma` (collection; e.g. playlist)
/// 
/// A few things can be noticed:
/// - An initial character of "i" could stand for "item", and "l" for "list"?
///    - This doesn't apply to the collections, but collections are already technically a list
///      by themselves, albeit not in the same form as the other container lists here, since
///      collections store their items using [`Boma`]s.
///    - An uppercase "L" is used for the accounts collection.
///      Notably, the accounts collection is not always present on a file.
/// 
/// The meaning behind the "ma" in signatures is not known.
///  - "Music Application"?: This has been around since iTunes, so I dunno.
/// 
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Signature([u8; Self::LENGTH]);
impl Signature {
    pub const LENGTH: usize = 4;

    pub fn empty_buffer() -> [u8; Self::LENGTH] {
        [0; Self::LENGTH]
    }

    /// Creates a new signature from the given four bytes.
    pub const fn new(bytes: [u8; Self::LENGTH]) -> Self {
        Self(bytes)
    }

    /// Returns the bytes for this signature.
    pub const fn bytes(&self) -> &[u8; Self::LENGTH] {
        &self.0
    }

    /// Returns this signature as a string.
    pub fn into_lossy_str<'a>(&'a self) -> ::std::borrow::Cow<'a, str> {
        ::std::string::String::from_utf8_lossy(&self.0)
    }
}
impl AsRef<[u8; Signature::LENGTH]> for Signature {
    fn as_ref(&self) -> &[u8; Signature::LENGTH] {
        self.bytes()
    }
}
impl core::fmt::Display for Signature {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.into_lossy_str())
    }
}
impl core::fmt::Debug for Signature {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // f.debug_tuple("Signature")
        //     .field(&self.into_lossy_str())
        //     .field(&self.bytes())
        //     .finish()
        write!(f, "{:?}", self.into_lossy_str())
    }
}
impl PartialEq<[u8; Signature::LENGTH]> for Signature {
    fn eq(&self, other: &[u8; Signature::LENGTH]) -> bool {
        self.0 == *other
    }
}
impl PartialEq<Signature> for [u8; Signature::LENGTH] {
    fn eq(&self, other: &Signature) -> bool {
        *self == other.0
    }
}
impl PartialEq<Signature> for &[u8; Signature::LENGTH] {
    fn eq(&self, other: &Signature) -> bool {
        **self == other.0
    }
}
impl PartialEq<Signature> for &[u8] {
    fn eq(&self, other: &Signature) -> bool {
        self.len() == Signature::LENGTH && self == other.bytes()
    }
}

pub trait ReadableChunk<'a>: Chunk {
    type ReadError: core::error::Error + From<std::io::Error>;

    /// Skips this chunk if the cursor is placed upon the correct signature.
    /// Returns `false` if the signature does not match.
    /// Returns `true` if the signature matches and the chunk was skipped.
    fn skip(cursor: &mut ChunkCursor<'a>) -> Result<bool, std::io::Error>;

    /// Reads the chunk if the cursor is placed upon the correct signature.
    /// Returns `None` if the signature does not match.
    fn read_optional(cursor: &mut ChunkCursor<'a>) -> Result<Option<Self>, Self::ReadError> where Self: Sized {
        let signature = cursor.peek_signature()?;
        if signature != Self::SIGNATURE { return Ok(None); }
        Self::read(cursor).map(Some)
    }

    /// Reads the signature and asserts that it matches the expected signature. This advances the cursor past the signature.
    /// Panics if the signature does not match.
    fn read_signature(cursor: &mut ChunkCursor<'a>) -> Result<(), Self::ReadError> where Self: Sized {
        let signature = cursor.peek_signature()?;
        assert!(signature == Self::SIGNATURE, "invalid header @ 0x{:X} ({}), expected {:?} got {signature:?}",
            cursor.position(),
            cursor.position(),
            Self::SIGNATURE,
        );

        cursor.advance(Signature::LENGTH as i64)?;
        Ok(())
    }

    /// Reads the chunk (signature and contents) and returns it.
    fn read(cursor: &mut ChunkCursor<'a>) -> Result<Self, Self::ReadError> where Self: Sized;
}

#[derive(Debug)]
struct DidNotReadToEndError {
    signature: Signature,
    offset: usize,
    length: u32,
    expected_end: usize,
    actual_end: usize,
}
impl core::error::Error for DidNotReadToEndError {}
impl core::fmt::Display for DidNotReadToEndError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "chunk did not read to end (signature: {}, offset: 0x{:X}, length: {}, expected end: 0x{:X}, actual end: 0x{:X})",
            self.signature,
            self.offset,
            self.length,
            self.expected_end,
            self.actual_end,
        )
    }
}


/// Certain chunks have associated data that extends beyond what their "length" dictates.
/// 
/// These are called "appendages", and their lengths are specified in different ways depending on the chunk,
/// though the relevant data will always be right after the length general field.
/// 
/// The reading of the lengths of these appendages has been abstracted so that we can check
/// that we haven't under-read or over-read any important data (see: [`LengthEnforcement`]).
pub mod appendage {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum LengthFollower {
        None,
        Quantity,
        ByteLength,
        ByteLengthAndQuantity,
    }
    impl LengthFollower {
        pub const fn has_byte_length_immediately(self) -> bool {
            matches!(self, Self::ByteLength | Self::ByteLengthAndQuantity)
        }
    }

    pub trait LengthFollowerVariantPossessor {
        const VARIANT: LengthFollower;
        fn read(cursor: &mut super::ChunkCursor<'_>) -> Self where Self: Sized;
        fn get_byte_length_inclusive(&self) -> Option<u32> { None }
    }

    pub mod lengths {
        use super::LengthFollowerVariantPossessor;
        use byteorder::ReadBytesExt;

        pub struct NoAppendage {}
        pub struct AppendageQuantity { pub count: u32 }
        pub struct LengthWithAppendages { pub appendage_inclusive_byte_length: u32 }
        pub struct LengthWithAppendagesAndQuantity { pub appendage_inclusive_byte_length: u32, pub count: u32 }

        impl LengthFollowerVariantPossessor for NoAppendage {
            const VARIANT: super::LengthFollower = super::LengthFollower::None;
            fn read(_cursor: &mut crate::chunk::ChunkCursor<'_>) -> Self where Self: Sized {
                Self {}
            }
        }
        impl LengthFollowerVariantPossessor for AppendageQuantity {
            const VARIANT: super::LengthFollower = super::LengthFollower::Quantity;
            fn read(cursor: &mut crate::chunk::ChunkCursor<'_>) -> Self where Self: Sized {
                let count = cursor.read_u32::<byteorder::LittleEndian>().unwrap();
                Self { count }
            }
        }
        impl LengthFollowerVariantPossessor for LengthWithAppendages {
            const VARIANT: super::LengthFollower = super::LengthFollower::ByteLength;
            fn read(cursor: &mut crate::chunk::ChunkCursor<'_>) -> Self where Self: Sized {
                let appendage_inclusive_byte_length = cursor.read_u32::<byteorder::LittleEndian>().unwrap();
                Self { appendage_inclusive_byte_length }
            }
            fn get_byte_length_inclusive(&self) -> Option<u32> {
                Some(self.appendage_inclusive_byte_length)
            }
        }
        impl LengthFollowerVariantPossessor for LengthWithAppendagesAndQuantity {
            const VARIANT: super::LengthFollower = super::LengthFollower::ByteLengthAndQuantity;
            fn read(cursor: &mut crate::chunk::ChunkCursor<'_>) -> Self where Self: Sized {
                let appendage_inclusive_byte_length = cursor.read_u32::<byteorder::LittleEndian>().unwrap();
                let count = cursor.read_u32::<byteorder::LittleEndian>().unwrap();
                Self { appendage_inclusive_byte_length, count }
            }
            fn get_byte_length_inclusive(&self) -> Option<u32> {
                Some(self.appendage_inclusive_byte_length)
            }
        }
    }
}

pub enum LengthEnforcement {
    ToDefinedLength,
    ToDefinedLengthPlusAppendagesByteLength,
    AtLeastDefinedLength, // for when appendage byte length isn't known
    None,
}

/// Utility trait for chunks that specify their fixed size as the first four bytes of their content immediately after the signature.
pub trait SizedFirstReadableChunk<'a>: Chunk {
    type ReadError: core::error::Error + From<std::io::Error>;
    type AppendageLengths: appendage::LengthFollowerVariantPossessor;
    const LENGTH_ENFORCED: LengthEnforcement = LengthEnforcement::ToDefinedLengthPlusAppendagesByteLength;
    /// Read the sized content of the chunk.
    /// The passed information is useful for using the [`setup_eaters!`] macro.
    /// ## Arguments
    /// - `cursor`: the cursor to read from; notably, it has already read eight bytes past `offset` (the signature and the chunk length)
    /// - `offset`: the byte index of the first character of the signature
    /// - `length`: the byte length of the main (non-appendage) content to read
    /// - `appendage_lengths`: the pre-read lengths of any appendages present after the main content
    fn read_sized_content(cursor: &mut super::chunk::ChunkCursor<'a>, offset: usize, length: u32, appendage_lengths: &Self::AppendageLengths) -> Result<Self, Self::ReadError> where Self: Sized;
}
impl<'a, T: SizedFirstReadableChunk<'a>> ReadableChunk<'a> for T {
    type ReadError = <Self as SizedFirstReadableChunk<'a>>::ReadError;

    fn skip(cursor: &mut super::chunk::ChunkCursor<'a>) -> Result<bool, std::io::Error> {
        use crate::chunk::appendage::LengthFollowerVariantPossessor;
        let signature = cursor.peek_signature()?;
        if signature != Self::SIGNATURE { return Ok(false); }
        cursor.advance(Signature::LENGTH as i64)?;
        let length = cursor.read_u32::<LittleEndian>()?;
        let length_including_appendages = if <Self as SizedFirstReadableChunk<'a>>::AppendageLengths::VARIANT.has_byte_length_immediately() { cursor.read_u32::<LittleEndian>()? } else { length };
        cursor.advance(length_including_appendages as i64)?;
        Ok(true)
    }

    fn read(cursor: &mut super::chunk::ChunkCursor<'a>) -> Result<Self, Self::ReadError> where Self: Sized {
        use core::cmp::Ordering;
        use crate::chunk::appendage::LengthFollowerVariantPossessor;

        let offset = cursor.position();
        Self::read_signature(cursor)?;
        let length = cursor.read_u32::<LittleEndian>()?;
        let appendage_lengths = <Self as SizedFirstReadableChunk<'a>>::AppendageLengths::read(cursor);
        let read = Self::read_sized_content(cursor, offset, length, &appendage_lengths)?;

        if let Some((equalities, enforced_length)) = match Self::LENGTH_ENFORCED {
            LengthEnforcement::ToDefinedLength => Some((&[Ordering::Equal] as &[Ordering], length as usize)),
            LengthEnforcement::ToDefinedLengthPlusAppendagesByteLength => {
                let Some(length_including_appendages) = appendage_lengths.get_byte_length_inclusive() else { panic!("cannot enforce length including appendages when appendage length is not known") };
                Some((&[Ordering::Equal] as &[Ordering], length_including_appendages as usize))
            },
            LengthEnforcement::AtLeastDefinedLength => Some((&[Ordering::Greater, Ordering::Equal] as &[Ordering], length as usize)),
            LengthEnforcement::None => None,
        } {
            if !equalities.iter().any(|ord| cursor.position().cmp(&(offset + enforced_length)) == *ord) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    DidNotReadToEndError {
                        signature: Self::SIGNATURE,
                        offset,
                        length,
                        expected_end: offset + enforced_length,
                        actual_end: cursor.position(),
                    }
                ).into());
            }
        }
        
        Ok(read)
    }
}
