#![doc = include_str!("../README.md")]
pub(crate) type Utf16Str = unaligned_u16::utf16::Utf16Str<unaligned_u16::endian::LittleEndian>;

#[cfg(any(test, feature = "tracing-subscriber"))]
pub fn setup_tracing_subscriber() {
    tracing_subscriber::fmt::init();
}

#[cfg(feature = "cli")]
pub mod cli;

pub mod chunk;
mod chunks;
pub mod packed;

pub mod id;
pub mod boma;
pub mod units;
pub use id::*;
mod version;
use boma::*;
use chunk::*;
pub use chunks::*;

pub(crate) fn convert_timestamp(seconds: u32) -> Option<chrono::DateTime<chrono::Utc>> {
    if seconds == 0 { return None }
    
    use chrono::TimeZone;

    const EPOCH_OFFSET: i64 = 2082819600;

    Some(chrono::Utc.timestamp_opt(seconds as i64 - EPOCH_OFFSET, 0).unwrap())
}

#[derive(Debug)]
struct HeaderRepeat {}
impl Chunk for HeaderRepeat {
    const SIGNATURE: Signature = Signature::new(*b"hfma");
}
impl<'a> ReadableChunk<'a> for HeaderRepeat {
    type ReadError = std::io::Error;

    fn skip(cursor: &mut ChunkCursor<'a>) -> Result<bool, std::io::Error> {
        use byteorder::ReadBytesExt;
        let offset = cursor.position();
        let signature = cursor.peek_signature()?;
        if signature != Self::SIGNATURE { return Ok(false); }
        let length = cursor.read_u32::<byteorder::LittleEndian>()?;
        cursor.set_position(offset + length as usize)?;
        Ok(true)
    }

    fn read(cursor: &mut ChunkCursor<'a>) -> Result<Self, Self::ReadError> where Self: Sized {
        use byteorder::ReadBytesExt;
        let offset = cursor.position();
        Self::read_signature(cursor)?;
        let length = cursor.read_u32::<byteorder::LittleEndian>()?;
        cursor.set_position(offset + length as usize)?;
        Ok(Self {})
    }
}

#[derive(Debug)]
pub struct MusicDbView<'a> {
    pub library: LibraryMaster<'a>,
    pub albums: AlbumMap<'a>,
    pub artists: ArtistMap<'a>,
    /// All of the Apple Music accounts associated with the storage.
    // Wasn't present on a Windows copy, but that might be because they've only logged in as one user.
    // For some god-forsaken reason beyond any comprehension, my personal laptop has had *two* associated
    // accounts, one of whom is a rapper and DJ from the UK? So, uh, needs more research.
    pub accounts: Option<AccountInfoList<'a>>,
    pub tracks: TrackMap<'a>,
    /// Playlists and other collections of tracks.
    pub collections: CollectionList<'a>
}
impl<'a> MusicDbView<'a> {
    pub(crate) fn with_cursor(mut cursor: chunk::ChunkCursor<'a>) -> Self {
        macro_rules! expect_boundary {
            ($cursor: ident) => {
                chunks::SectionBoundary::<u32>::read(&mut $cursor).expect("can't read section boundary");        
            }
        }

        expect_boundary!(cursor);
        HeaderRepeat::read(&mut cursor).expect("can't read header duplicate");

        expect_boundary!(cursor);
        let library = LibraryMaster::read(&mut cursor).expect("can't read library master");
        
        expect_boundary!(cursor);
        let albums = AlbumMap::read(&mut cursor).expect("can't read albums list");

        expect_boundary!(cursor);
        let artists = ArtistMap::read(&mut cursor).expect("can't read artists list");

        expect_boundary!(cursor);
        let accounts = AccountInfoList::read_optional(&mut cursor).expect("can't read account list");

        if accounts.is_some() { expect_boundary!(cursor); }
        let tracks = TrackMap::read(&mut cursor).expect("can't read track list");

        expect_boundary!(cursor);
        let collections = CollectionList::read(&mut cursor).expect("can't read collection list");

        Self {
            library,
            albums,
            artists,
            accounts,
            tracks,
            collections
        }
    }

    /// Returns the value with the given ID (be it a track, album, artist, et cetera).
    /// 
    /// Only works for IDs with their datatype attached at the type-level, such as IDs which were retrieved from the DB itself.
    #[allow(clippy::missing_transmute_annotations)]
    pub fn get<T: id::persistent::Possessor>(&self, id: PersistentId<T>) -> Option<&'a T> {
        match T::IDENTITY {
            id::persistent::PossessorIdentity::Account => {
                let id: PersistentId<Account<'a>> = unsafe { core::mem::transmute(id) };
                #[cfg(feature = "tracing")]
                if self.accounts.is_none() { tracing::warn!("account ID passed without existence of accounts field"); };
                let account = self.accounts.as_ref().and_then(|accounts| {
                    accounts.iter().find(|account| account.persistent_id == id)
                 });
                unsafe { core::mem::transmute(account) }
            }
            id::persistent::PossessorIdentity::Album => {
                let id: PersistentId<Album<'a>> = unsafe { core::mem::transmute(id) };
                let album = self.albums.get(&id);
                unsafe { core::mem::transmute(album) }
            },
            id::persistent::PossessorIdentity::Artist => {
                let id: PersistentId<Artist<'a>> = unsafe { core::mem::transmute(id) };
                let artist = self.artists.get(&id);
                unsafe { core::mem::transmute(artist) }
            },
            id::persistent::PossessorIdentity::Collection => {
                let id: PersistentId<Collection<'a>> = unsafe { core::mem::transmute(id) };
                let collection = &self.collections.0.iter().find(|collection| collection.persistent_id == id);
                unsafe { core::mem::transmute(collection) }
            },
            id::persistent::PossessorIdentity::Track => {
                let id: PersistentId<Track<'a>> = unsafe { core::mem::transmute(id) };
                let track = self.tracks.get(&id);
                unsafe { core::mem::transmute(track) }
            },
        }
    }
}
macro_rules! impl_db_collection_coercion {
    ($coerce_to: ident, $field: ident) => {
        impl<'a> From<&'a MusicDbView<'a>> for &'a $coerce_to<'a> {
            fn from(value: &'a MusicDbView<'a>) -> Self {
                &value.$field
            }
        }
        impl<'a> From<&'a MusicDB> for &'a $coerce_to<'a> {
            fn from(value: &'a MusicDB) -> Self {
                &value.get_view().$field
            }
        }
    };
}
impl_db_collection_coercion!(AlbumMap, albums);
impl_db_collection_coercion!(ArtistMap, artists);
impl_db_collection_coercion!(TrackMap, tracks);
impl_db_collection_coercion!(CollectionList, collections);

pub struct MusicDB {
    view: MusicDbView<'static>, // not really static; lifetime is 'self (as long as `_owned_data` exists)
    path: std::path::PathBuf,
    _owned_data: core::pin::Pin<Box<[u8]>>,
}

impl MusicDB {
    pub fn read_path(path: impl AsRef<std::path::Path>) -> Result<MusicDB, packed::UnpackError> {
        let decoded = Self::unpack(&path)?;
        Ok(Self::from_unpacked(decoded.into_boxed_slice(), path))
    }
    /// Construct a MusicDB from already-unpacked data. This still expects that the data at `path` is packed.
    pub fn from_unpacked(data: Box<[u8]>, path: impl AsRef<std::path::Path>) -> MusicDB {
        let path = path.as_ref().to_path_buf();
        let data = core::pin::Pin::new(data);

        // Obtain a slice of the data with a lifetime promoted to that of the returned instance (not actually 'static, but 'self).
        // SAFETY:
        //  - The data is behind [`core::pin::Pin`], meaning the memory address won't ever change.
        //  - The data will be owned by the returned struct, so if it's dropped, the view would already be invalidated as the lifetime would've expired.
        //  - The data is contiguous, and can safely be mapped to a slice.
        let slice: &'static [u8] = unsafe {
            core::slice::from_raw_parts::<'static, u8>(data.as_ptr(), data.len())
        };

        let cursor = chunk::ChunkCursor::new(slice);
        let view = MusicDbView::with_cursor(cursor);

        Self { view, path, _owned_data: data }
    }
    /// Unpacks the `.musicdb` file at the given path, returning the raw contents.
    pub fn unpack(path: impl AsRef<std::path::Path>) -> Result<Vec<u8>, packed::UnpackError> {
        Ok(packed::unpack_in_place(&mut std::fs::read(&path)?)?.0)
    }
    pub fn get_raw(&self) -> &[u8] {
        &self._owned_data
    }
    pub fn get_view<'a>(&self) -> &MusicDbView<'a> {
        // 'static => 'self
        unsafe { core::mem::transmute(&self.view) }
    }
    // TODO: Remove this method; I don't like this struct being in any way mutable since it shares the view.
    pub fn get_view_mut(&mut self) -> &mut MusicDbView<'_> {
        // 'static => 'self
        unsafe { core::mem::transmute(&mut self.view) }
    }
    /// Updates the view by re-reading/decoding the file from disk.
    pub fn update_view(&mut self) -> Result<(), packed::UnpackError> {
        *self = Self::read_path(self.path.as_path())?;
        Ok(())
    }
    pub fn default_path() -> std::path::PathBuf {
        std::env::home_dir().expect("no user home directory detected").as_path()
            .join("Music/Music/Music Library.musiclibrary/Library.musicdb")
    }
}
impl core::default::Default for MusicDB {
    fn default() -> Self {
        MusicDB::read_path(MusicDB::default_path()).expect("failed to read current user's musicdb")
    }
}
impl core::fmt::Debug for MusicDB {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MusicDB")
            .field("path", &self.path)
            .field("view", &self.view)
            .finish()
    }
}
impl MusicDB {
    /// Returns the value with the given ID (be it a track, album, artist, et cetera).
    /// 
    /// Only works for IDs with their datatype attached at the type-level, such as IDs which were retrieved from the DB itself.
    pub fn get<T: id::persistent::Possessor>(&self, id: PersistentId<T>) -> Option<&T> {
        self.get_view().get(id)
    }

    /// Returns a map of every album in the library.
    pub fn albums(&self) -> &AlbumMap<'_> {
        &self.get_view().albums
    }
    /// Returns a map of every artist in the library.
    pub fn artists(&self) -> &ArtistMap<'_> {
        &self.get_view().artists
    }
    /// Returns a map of every track in the library.
    pub fn tracks(&self) -> &TrackMap<'_> {
        &self.get_view().tracks
    }
    /// Returns a map of every collection of tracks in the library, such as playlists or (potentially internal) groupings.
    pub fn collections(&self) -> &CollectionList<'_> {
        &self.get_view().collections
    }
    /// Returns a map of every account associated with the library.
    /// This isn't always present.
    pub fn accounts(&self) -> Option<&AccountInfoList<'_>> {
        self.get_view().accounts.as_ref()
    }
}

#[test]
// #[ignore = "needs populated samples directory"]
fn try_all_samples() {
    crate::setup_tracing_subscriber();

    fn process_dir(path: &std::path::Path) {
        for entry in std::fs::read_dir(path).expect("fs error") {
            let entry = entry.expect("fs error");
            let path = entry.path();
            if path.is_dir() { process_dir(&path); return; }
            match path.extension().and_then(|s| s.to_str()) {
                Some("musicdb") => {
                    tracing::info!("processing sample file: {}", path.display());
                    if let Err(error) = MusicDB::read_path(&path) {
                        tracing::error!(?path, ?error, "failed to read / decode sample");
                    } else {
                        tracing::info!(?path, "successfully read and decoded sample");
                    }
                },
                Some("decoded") => {
                    tracing::info!("processing sample file: {}", path.display());
                    let decoded = std::fs::read(&path).expect("fs error");
                    let _ = MusicDB::from_unpacked(decoded.into_boxed_slice(), &path);
                    tracing::info!(?path, "successfully read pre-decoded sample");
                }
                _ => {}
            }
        }
    }

    process_dir(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("samples"));
}

#[allow(unused)]
pub(crate) fn xxd(mut slice: &[u8]) -> String {
    let mut out = String::new();
    const HEX_PER_LINE: usize = 32;
    let mut n = 0;
    while !slice.is_empty() {
        let (line, rest) = slice.split_at(slice.len().min(HEX_PER_LINE));
        slice = rest;

        const ZERO_PADDING: usize = 4;
        let row = n * HEX_PER_LINE; n += 1;        
        let digits = ((row as f64).log10().ceil() as usize).max(1);
        if digits != 0 {
            out.push_str("\x1b[2;30m"); // light gray
            for digit in 0..ZERO_PADDING - digits {
                out.push('0');
            }
            out.push_str("\x1b[0m"); // reset
        }

        out.push_str(&format!("{row} | "));


        for byte in line {
            if *byte == 0 {
                out.push_str("\x1b[2;30m00 \x1b[0m")
            } else {
                out.push_str(&format!("{byte:02x} "));
            }
        }

        out.push('\n');
    }
    out
}
