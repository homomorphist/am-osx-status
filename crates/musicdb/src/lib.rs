#![doc = include_str!("../README.md")]
use std::{fmt::Debug, io::Cursor, path::Path, pin::Pin};
pub(crate) type Utf16Str = unaligned_u16::utf16::Utf16Str<unaligned_u16::endian::LittleEndian>;


pub mod chunk;
mod chunks;
pub mod encoded;

pub mod id;
pub mod boma;
pub mod units;
pub use id::*;
mod version;
use boma::*;
use chunk::*;
pub use chunks::*;

#[cfg(not(feature = "tracing"))]
#[allow(unused)]
mod tracing {
    // mock
    pub struct Span;
    impl Span {
        pub fn in_scope<T>(self, f: impl FnOnce() -> T) -> T {
            f()
        }
    }

    macro_rules! debug_span {
        ($name: expr) => {
            tracing::Span
        };
    }
    macro_rules! _warn {
        ($($arg: tt)*) => {};
    }
    
    pub(crate) use debug_span;
    pub(crate) use _warn as warn;
}

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
impl<'a> SizedFirstReadableChunk<'a> for HeaderRepeat {
    type ReadError = std::io::Error;
    fn read_sized_content(cursor: &mut Cursor<&'a [u8]>, offset: u64, length: u32) -> Result<Self, Self::ReadError> where Self: Sized {
        setup_eaters!(cursor, offset, length);
        skip_to_end!()?; // skip the rest of the section
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
    pub collections: CollectionMap<'a>
}
impl<'a> MusicDbView<'a> {
    pub(crate) fn with_cursor(mut cursor: Cursor<&'a [u8]>) -> Self {
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
        let collections = CollectionMap::read(&mut cursor).expect("can't read collection list");

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
                if self.accounts.is_none() {
                    tracing::warn!("account ID passed without existence of accounts field");
                };
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
impl<'a> From<&'a MusicDbView<'a>> for &'a AlbumMap<'a> {
    fn from(value: &'a MusicDbView<'a>) -> Self {
        &value.albums
    }
}
impl<'a> From<&'a MusicDbView<'a>> for &'a ArtistMap<'a> {
    fn from(value: &'a MusicDbView<'a>) -> Self {
        &value.artists
    }
}
impl<'a> From<&'a MusicDbView<'a>> for &'a TrackMap<'a> {
    fn from(value: &'a MusicDbView<'a>) -> Self {
        &value.tracks
    }
}
impl<'a> From<&'a MusicDbView<'a>> for &'a CollectionMap<'a> {
    fn from(value: &'a MusicDbView<'a>) -> Self {
        &value.collections
    }
}

pub struct MusicDB {
    _owned_data: Pin<Vec<u8>>,
    view: MusicDbView<'static>, // not really static; lifetime is 'self (as long as `_owned_data` exists)
    path: std::path::PathBuf
}

impl MusicDB {
    pub fn read_path(path: impl AsRef<Path>) -> MusicDB {
        let path = path.as_ref().to_path_buf();
        let data = &mut std::fs::read(&path).unwrap()[..];
        let (decoded, _) = encoded::decode_in_place(data).unwrap();
        let data = Pin::new(decoded);

        // Obtain a slice of the data with a lifetime promoted to that of the returned instance (not actually 'static, but 'self).
        // SAFETY:
        //  - The data is behind [`core::pin::Pin`], meaning the memory address won't ever change.
        //  - The data will be owned by the returned struct, so if it's dropped, the view would already be invalidated as the lifetime would've expired.
        //  - The data is contiguous, and can safely be mapped to a slice.
        let slice: &'static [u8] = unsafe {
            let addr = data.as_ptr();
            core::slice::from_raw_parts::<'static, u8>(addr, data.len())
        };

        let cursor = Cursor::new(slice);
        let view = MusicDbView::with_cursor(cursor);

        Self { view, _owned_data: data, path }
    }
    pub fn extract_raw(path: impl AsRef<Path>) -> Result<Vec<u8>, std::io::Error> {
        let data = &mut std::fs::read(&path)?;
        let (decoded, _) = encoded::decode_in_place(data).unwrap();
        Ok(decoded)
    }
    pub fn get_view(&self) -> &MusicDbView<'_> {
        // 'static => 'self
        unsafe { core::mem::transmute(&self.view) }
    }
    pub fn get_view_mut(&mut self) -> &mut MusicDbView<'_> {
        // 'static => 'self
        unsafe { core::mem::transmute(&mut self.view) }
    }
    pub fn update_view(&mut self)  {
        // TODO: Persistent handle? I dunno.
        *self = Self::read_path(self.path.as_path())
    }
    pub fn default_path() -> std::path::PathBuf {
        std::env::home_dir().expect("no user home directory detected").as_path()
            .join("Music/Music/Music Library.musiclibrary/Library.musicdb")
    }
}
impl core::default::Default for MusicDB {
    fn default() -> Self {
        MusicDB::read_path(MusicDB::default_path())
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
    /// Returns a map of every collection (playlist) in the library.
    pub fn collections(&self) -> &CollectionMap<'_> {
        &self.get_view().collections
    }
    /// Returns a map of every account associated with the library.
    /// This isn't always present.
    pub fn accounts(&self) -> Option<&AccountInfoList<'_>> {
        self.get_view().accounts.as_ref()
    }
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
