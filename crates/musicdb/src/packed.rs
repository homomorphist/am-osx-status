//! Packed form: encrypted & compressed
//! <hr>
//! 
//! A `.musicdb` file holds only one [`Chunk`](crate::chunk::Chunk), that being the ["hfma" chunk](`Container`), which serves as a container for the actual data being stored.
//! 
//! It must undergo decryption and decompression before being usable.
//! The resulting size of this operation is not stored within the file, so an initial allocation is done with a heuristic multiplier of 8 times the size of the compressed data.
//! 
//! ## Format
//! 
//! The data undergoes two transformations before being stored in the file:
//!  1. It is compressed following the DEFLATE algorithm.
//!  2. It is encrypted using AES-128 in ECB mode.
//! 
//! ### Partial Encryption
//! 
//! There are two situations in which unencrypted (though still compressed) data may be appended at the end of the encrypted data:
//!  1. If the last bit of data cannot fit into a full chunk of sixteen bytes.
//!  2. If the amount of bytes encrypted has exceeded a [defined threshold in the file header](`Container`).
//! 
use std::io::Read;

use crate::chunk::{ReadableChunk, Signature, SizedFirstReadableChunk};

/// A moderately-upper-end guess on how much larger the unpacked data will be compared to the packed form.
// A value can be computed on local files with [`crate::cli::Ratios`].
pub const EXPANDED_SIZE_MULTIPLIER_HEURISTIC: usize = 8;

/// A key used to decrypt the iTunes and Apple Music library files, [known publicly since at least 2010][kafsemo].
/// 
/// This key does not have any known usage in decrypting copyrighted or DRM-protected media,
/// and is used solely to obtain the contents of a user's library, information which is already
/// accessible to the user through the iTunes or Apple Music applications themselves.
/// 
/// [kafsemo]: <https://kafsemo.org/2010/12/10_itunes-10-database.html>
pub const KEY: &[u8] = b"BHUILuilfghuila3";

#[derive(thiserror::Error, Debug)]
pub enum UnpackError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("decryption failure: {0}")]
    Decryption(aes::cipher::block_padding::UnpadError),
    #[error("decompression failure: {0}")]
    Decompression(std::io::Error)
}

pub fn unpack_in_place<'a>(data: &'a mut [u8]) -> Result<(Vec<u8>, PackedFileInfo<'a>), UnpackError> {
    let mut data = core::cell::UnsafeCell::new(data);
    let mut cursor = super::chunk::ChunkCursor::new({
        // SAFETY: This data won't get mutated; the header is preserved and we only apply the decryption in-place on the encrypted data.
        // We would've used `core::slice::split_at_mut` but we don't know the size of the header ahead of time.
        unsafe { &**data.get() }
    });

    let info = PackedFileInfo::read(&mut cursor).map_err(UnpackError::Io)?;
    let data = &mut data.get_mut()[info.header_size as usize..];
    let split_at = (info.max_encrypted_byte_count as usize).min(data.len() & !0x0F);

    Ok((decode_split_encryption(data, split_at)?, info))
}

fn decode_split_encryption(data: &mut [u8], at: usize) -> Result<Vec<u8>, UnpackError> {
    let (encrypted, unencrypted) = data.split_at_mut(at);
    let decrypted = decrypt_in_place(encrypted).map_err(UnpackError::Decryption)?;
    let compressed = ReadableDualJoined::new(decrypted, unencrypted);
    let compressed_length = compressed.len();
    decompress(compressed, compressed_length).map_err(UnpackError::Decompression)
}

#[cfg_attr(feature = "tracing", tracing::instrument(level = "debug", skip(bytes)))]
fn decrypt_in_place(bytes: &mut [u8]) -> Result<&mut [u8], aes::cipher::block_padding::UnpadError> {
    use ecb::cipher::{KeyInit, BlockDecryptMut};
    type Padding = aes::cipher::block_padding::NoPadding;
    type Decryptor = ecb::Decryptor<aes::Aes128>;
    Decryptor::new(KEY.into()).decrypt_padded_mut::<Padding>(bytes)?;
    Ok(bytes)
}

#[cfg_attr(feature = "tracing", tracing::instrument(level = "debug", skip(source)))]
fn decompress(source: impl Read, compressed_size: usize) -> Result<Vec<u8>, std::io::Error> {
    use flate2::read::ZlibDecoder;
    let mut decompressed = Vec::with_capacity(compressed_size * EXPANDED_SIZE_MULTIPLIER_HEURISTIC);
    ZlibDecoder::new(source).read_to_end(&mut decompressed)?;
    Ok(decompressed)
}


/// Read from two slices, one after the other, without allocating.
struct ReadableDualJoined<'a> {
    second: &'a [u8],
    current: &'a [u8],
    index: usize,
}
impl<'a> ReadableDualJoined<'a> {
    fn new(a: &'a [u8], b: &'a [u8]) -> Self {
        Self { current: a, second: b, index: 0 }
    }

    fn len(&self) -> usize {
        self.current.len() + self.second.len()
    }
}
impl Read for ReadableDualJoined<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if self.index == self.current.len() {
            if self.current == self.second {
                return Ok(0);
            } else {
                self.current = self.second;
                self.index = 0;
            }
        }
        let size = buffer.len();
        let read = size.min(self.current.len() - self.index);
        buffer[..read].copy_from_slice(&self.current[self.index..][..read]);
        self.index += read;
        Ok(read)
    }
}

#[allow(unused)]
#[derive(Debug)]
pub struct PackedFileInfo<'a> {
    header_size: u32,
    max_encrypted_byte_count: u32,

    pub app_version: &'a core::ffi::CStr,

    track_count: u32,
    playlist_count: u32,
    collection_count: u32,
    artist_count: u32,
}
impl crate::chunk::Chunk for PackedFileInfo<'_> {
    const SIGNATURE: Signature = Signature::new(*b"hfma");
}
impl<'a> SizedFirstReadableChunk<'a> for PackedFileInfo<'a> {
    type ReadError = std::io::Error;
    type AppendageLengths = crate::chunk::appendage::lengths::LengthWithAppendages;
    const LENGTH_ENFORCED: crate::chunk::LengthEnforcement = crate::chunk::LengthEnforcement::ToDefinedLength;
    fn read_sized_content(cursor: &mut crate::chunk::ChunkCursor<'a>, offset: usize, header_size: u32, _: &Self::AppendageLengths) -> Result<Self, Self::ReadError> where Self: Sized {
        crate::chunk::setup_eaters!(cursor, offset, header_size);
        let _format_major = u16!()?;
        let _format_minor = u16!()?;
        let app_version = cstr!(0x20)?;
        let _persistent_id = u64!()?;
        let _file_variant = u32!()?;
        skip!(4)?; // ?
        skip!(4)?; // ?
        let track_count = u32!()?;
        let playlist_count = u32!()?;
        let collection_count = u32!()?;
        let artist_count = u32!()?;
        let max_encrypted_byte_count = u32!()?;
        skip_to_end!()?;
        dbg!(cursor.position());

        Ok(Self {
            header_size,
            app_version,
            max_encrypted_byte_count,
            track_count,
            playlist_count,
            collection_count,
            artist_count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires the default path to point to a valid file"]
    fn test_decrypt() {
        let path = crate::MusicDB::default_path();
        let mut file = std::fs::File::open(path).expect("failed to open file");
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).expect("failed to read file");
        let original_size = bytes.len();
        let (decoded, _) = unpack_in_place(&mut bytes[..]).expect("failed to decode file");
        assert!(decoded.len() > original_size, "should have grown due to decompression");
    }    
}
