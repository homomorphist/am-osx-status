use std::io::{Seek, SeekFrom, Read, Cursor};
use byteorder::{LittleEndian, ReadBytesExt};

/// A chunk is a piece of data distinguished by a four-byte [signature](`Signature`) and a size.
// TODO: Mention nature of size, appendages, ...
pub trait Chunk {
    /// The signature for this chunk.
    const SIGNATURE: Signature;

    /// Returns the signature that this chunk uses.
    fn get_signature(&self) -> Signature {
        Self::SIGNATURE
    }
}

#[allow(unused)]
pub(crate) trait CursorReadingExtensions<'a>: Seek + Read + 'a {
    fn get_position(&self) -> u64;
    fn get_slice(&self) -> &'a [u8];
    fn get_slice_ahead(&self) -> &'a [u8] {
       &self.get_slice()[self.get_position() as usize..]
    }
    fn get_positioned_ptr(&self) -> *const u8 {
        self.get_slice_ahead().as_ptr()
    }


    fn read_signature(&mut self) -> Result<Signature, std::io::Error> {
        let signature = self.peek_signature()?;
        self.advance(Signature::LENGTH as i64)?;
        Ok(signature)
    }

    fn peek(&'a self, amount: usize) -> &'a [u8] {
        let slice = self.get_slice_ahead();
        &slice[..amount.min(slice.len())]
    }

    fn peek_exact<const N: usize>(&self) -> Result<&'a [u8; N], std::io::Error> {
        let ahead = self.get_slice_ahead();
        match ahead.get(..N) {
            Some(slice) => Ok(unsafe {
                *core::mem::transmute::<
                    &&'a [u8],
                    &&'a [u8; N]
                >(&slice)
            }),

            None => Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                format!("expected {} bytes, got {}", N, ahead.len()),
            ))
        }
    }

    fn read_slice(&mut self, amount: usize) -> Result<&'a [u8], std::io::Error> {
        let slice = self.get_slice_ahead();
        let read = slice.get(..amount).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                format!("expected {} bytes, got {}", amount, slice.len()),
            )
        })?;
        self.advance(read.len() as i64)?;
        Ok(read)
    }

    fn read_cstr_block<const N: usize>(&mut self) -> Result<&'a core::ffi::CStr, std::io::Error> {
        if !self.peek_exact::<{ N }>()?.contains(&0) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "string did not terminate in allocated block",
            ));
        }

        let ptr = self.get_positioned_ptr() as *const core::ffi::c_char;
        self.advance(N as i64)?;
        Ok(unsafe { core::ffi::CStr::from_ptr(ptr) })
    }

    fn peek_signature(&mut self) -> Result<Signature, std::io::Error> where Self: Read {
        self.peek_exact::<{ Signature::LENGTH }>()
            .map(|bytes| Signature::new(*bytes))
    }

    fn backtrack(&mut self, amount: i64) -> Result<u64, std::io::Error> {
        self.advance(-amount)
    }

    fn advance(&mut self, amount: i64) -> Result<u64, std::io::Error> {
        self.seek(SeekFrom::Current(amount))
    }

    fn reading_chunks<'b, T>(&'b mut self, amount: usize) -> ContiguousChunkReader<'b, 'a, T> where T: ReadableChunk<'a>;
}
impl<'a> CursorReadingExtensions<'a> for Cursor<&'a [u8]> {
    fn get_slice(&self) -> &'a [u8] {
        self.get_ref()
    }
    fn get_position(&self) -> u64 {
        self.position()
    }

    fn reading_chunks<'b, T>(&'b mut self, amount: usize) -> ContiguousChunkReader<'b, 'a, T> where T: ReadableChunk<'a> {
        ContiguousChunkReader::new(self, amount)
    }
}

pub(crate) struct ContiguousChunkReader<'a, 'b, T: ReadableChunk<'b>> {
    cursor: &'a mut Cursor<&'b [u8]>,
    remaining: usize,
    _type: core::marker::PhantomData<T>
}
impl<'a, 'b, T: ReadableChunk<'b>> ContiguousChunkReader<'a, 'b, T> {
    pub fn new(cursor: &'a mut Cursor<&'b [u8]>, remaining: usize) -> Self {
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
        use $crate::chunk::CursorReadingExtensions;
        #[allow(unused)] use byteorder::ReadBytesExt as _;
        $crate::chunk::setup_eaters!($cursor, $start_position, $length, ext: false);
    };
    ($cursor: ident, $start_position: ident, $length: ident, ext: false) => {
        #[allow(unused)] macro_rules! skip { ($count: expr) => { $cursor.advance($count) } }
        #[allow(unused)] macro_rules! skip_to_end { () => { $cursor.advance($length as i64 - ($cursor.position() - $start_position) as i64) } }
        #[allow(unused)] macro_rules! u64 { () => { $cursor.read_u64::<byteorder::LittleEndian>() } }
        #[allow(unused)] macro_rules! u32 { () => { $cursor.read_u32::<byteorder::LittleEndian>() } }
        #[allow(unused)] macro_rules! u16 { () => { $cursor.read_u16::<byteorder::LittleEndian>() } }
        #[allow(unused)] macro_rules!  u8 { () => { $cursor.read_u8() } }
        #[allow(unused)] macro_rules! cstr_block { ($size: literal) => {{ $cursor.read_cstr_block::<{ $size }>() }}}
        #[allow(unused)] macro_rules! id { ($type: ty) => {{ 
            $cursor.read_u64::<byteorder::LittleEndian>()
                .map($crate::id::persistent::Id::<$type>::new)
        }}}
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
    fn skip(cursor: &mut Cursor<&'a [u8]>) -> Result<bool, std::io::Error> {
        let signature = cursor.peek_signature()?;
        if signature != Self::SIGNATURE { return Ok(false); }
        cursor.advance(Signature::LENGTH as i64)?;
        let length = cursor.read_u32::<LittleEndian>()?;
        cursor.advance(length as i64)?;
        Self::skip_extras(cursor);
        Ok(true)
    }

    /// Reads the chunk if the cursor is placed upon the correct signature.
    /// Returns `None` if the signature does not match.
    fn read_optional(cursor: &mut Cursor<&'a [u8]>) -> Result<Option<Self>, Self::ReadError> where Self: Sized {
        let signature = cursor.peek_signature()?;
        if signature != Self::SIGNATURE { return Ok(None); }
        Self::read(cursor).map(Some)
    }

    /// Reads the signature and asserts that it matches the expected signature.
    /// Returns the original position of the cursor, or an error if a read error occurs.
    /// Panics if the signature does not match.
    fn read_signature(cursor: &mut Cursor<&'a [u8]>) -> Result<(), Self::ReadError> where Self: Sized {
        let signature = cursor.peek_signature()?;
        assert!(signature == Self::SIGNATURE, "invalid header @0x{:X} ({}), expected {:?} got {signature:?}",
            cursor.position(),
            cursor.position(),
            Self::SIGNATURE,
        );

        cursor.advance(Signature::LENGTH as i64)?;
        Ok(())
    }

    /// Reads the chunk and returns it.
    fn read(cursor: &mut Cursor<&'a [u8]>) -> Result<Self, Self::ReadError> where Self: Sized;

    /// Skips any appendages that this chunk may have.
    /// By default, this does nothing.
    fn skip_extras(_cursor: &mut Cursor<&'a [u8]>) {}
}

/// Utility trait for chunks that specify their fixed size as the first four bytes of their content immediately after the signature.
/// This is the case for almost every chunk, but a few odd-balls exist.
pub trait SizedFirstReadableChunk<'a>: Chunk {
    type ReadError: core::error::Error + From<std::io::Error>;

    /// Read the sized content of the chunk.
    /// The passed information is useful for using the [`setup_eaters!`] macro.
    /// ## Arguments
    /// - `cursor`: the cursor to read from; notably, it has already read eight bytes past `offset` (the signature and the chunk length)
    /// - `offset`: the byte index of the first character of the signature
    /// - `length`: the byte length of the content to read
    fn read_sized_content(cursor: &mut Cursor<&'a [u8]>, offset: u64, length: u32) -> Result<Self, Self::ReadError> where Self: Sized;

    /// Skips any appendages that this chunk may have.
    /// By default, this does nothing.
    fn skip_extras(_cursor: &mut Cursor<&'a [u8]>) {}
}
impl<'a, T: SizedFirstReadableChunk<'a>> ReadableChunk<'a> for T {
    type ReadError = <Self as SizedFirstReadableChunk<'a>>::ReadError;

    fn read(cursor: &mut Cursor<&'a [u8]>) -> Result<Self, Self::ReadError> where Self: Sized {
        let offset = cursor.position();
        Self::read_signature(cursor)?;
        let length = cursor.read_u32::<LittleEndian>()?;
        Self::read_sized_content(cursor, offset, length)
    }
    fn skip_extras(cursor: &mut Cursor<&'a [u8]>) {
        <Self as SizedFirstReadableChunk<'a>>::skip_extras(cursor);
    }
}
