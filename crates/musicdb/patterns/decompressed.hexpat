#define rel $ + addressof(this)

#include <std/io.pat>
#include <std/random.pat>
#include <type/magic.pat>

#pragma pattern_limit 2500000

bool process_album in;
bool process_album_boma in;

bool process_artist in;
bool process_artist_boma in;

bool process_account in;
bool process_account_boma in;

bool process_track in;
bool process_track_boma in;

bool process_collection in;
bool process_collection_boma in;


#define LIGHT_GRAY color("d3d3d3")

enum SectionBoundarySubtype: u32 {
    PlaylistMasterOrFileEntry = 3, // hsma, lPma
    LibraryMaster = 6, // plma
    AlbumList = 4, // lama
    ArtistList = 5, // lAma
    AccountData = 15, // Lsma
    TrackList = 1, // Ltma
    CollectionList = 2, // lPma
};

struct SectionBoundary {
    type::Magic<"hsma"> signature;
    u32 length; // to next
    u32 section_length;
    SectionBoundarySubtype subtype;
    padding[length - rel];
};


using PersistentID = u64; // Persistent ID
using MacTime = le u32; // todo pls explain

enum MusicDbFile: u32 {
    LibraryPreferences = 2,
    Application_Catalina = 4,
    Application_BigSur = 5,
    Library_Legacy = 6,
    Library_Sonoma = 7,
};


struct Header {
    type::Magic<"hfma"> signature;
    u32 length;
    padding[4];
    u16 format_major;
    u16 format_minor;
    char version[];
    padding[32 - sizeof(version)]; // trailing null
    PersistentID library_persistent_id;
    u32 unk_1 @ 0x74 [[LIGHT_GRAY]];
    u32 unk_2 @ 0x78 [[LIGHT_GRAY]];
    u64 unk_3 @ 0x90 [[LIGHT_GRAY]];
    u32 unk_4 @ 0x98 [[LIGHT_GRAY]];
    u32 unk_5 @ 0x9C [[LIGHT_GRAY]];
    u64 unk_6 @ 0xB0 [[LIGHT_GRAY]];
    padding[length - rel];
};


struct GenericList<Sig, T, auto skip, Skip> {
    Sig signature;
    u32 length;
    u32 item_count;
    padding[length - rel];

    if (skip) {
        Skip item[item_count];
    } else {
        T item[item_count];
    }
};


SectionBoundary bound0 @ $;
Header header @ $;
SectionBoundary bound1 @ $;

enum BomaSubtype: u32 {
    CollectionMember = 206,
    TrackNumerics = 0x1,
    TrackPlayStatistics = 0x17,
    
    // begin utf16
    TrackTitle = 0x2,
    Album = 0x3,
    Artist = 0x4,
    Genre = 0x5,
    Kind = 0x6,
    Comment = 0x8,
    Composer = 0xC,
    Grouping = 14,
    AlbumArtist = 0x1B,
    ClassicalWorkName = 63,
    ClassicalMovementTitle = 64,
    FairPlayInfo = 43,
    SortOrderTrackTitle = 0x1E,
    SortOrderAlbum = 0x1F,
    SortOrderArtist = 0x20,
    SortOrderAlbumArtist = 0x21,
    SortOrderComposer = 0x22,
    CopyrightHolder = 0x2E,
    TrackLocalFilePath = 67,
    PurchaserEmail = 0x3B,
    PurchaserName = 0x3C,
    PlaylistName = 200,
    IamaAlbum = 0x12C,
    IamaAlbumArtist = 0x12D,
    IamaAlbumArtistCloud = 0x12E, // not on local albums (maybe it would if it was a recognized music / available on apple music?)
    SeriesTitle = 0x12F,
    IamaAlbumCloudId = 0x133,
    ArtistsArtistName = 400,
    ArtistsArtistNameSorted = 401, // for use in sorted lists, e.x. without leading "The"
    ArtistsArtistCloudId = 403,
    AccountCloudId = 800, // `sp.{UUIDv4}`
    AccountDisplayName = 801,
    AccountUsername = 802,
    AccountUrlSafeId = 803, // used for album cover URL
    AccountAvatarUrl = 804,
    UnknownHex1 = 0x1F4,
    ManagedMediaFolder = 0x1F8,
    UnknownHex2 = 0x1FE,
    // end utf16
    // start utf8
    PlistTrackCloudInformation = 0x36,
    PlistCloudDownloadInformation = 0x38,
    PlistArtworkURL = 0x192,
    PlistPlaylistInfo = 0xCD,
    TrackLocalFilePathUrl = 11,
    // end utf8
    // start book
    BookVariant0 = 0x42,    
    BookVariant1 = 0x1FC,
    BookVariant2 = 0x1FD,
    BookVariant3 = 0x200,
    // idk
    Unknown1 = 23 // appears alongside downloaded tracks
    
};

enum BomaBookMemberType: u32 {
    PathComponent = 257,
    PathProtocol = 2305,
    SandboxInfo = 513
};


struct BomaBookMember {
    u32 length [[color("C3B299")]];
    BomaBookMemberType type [[color("D4A5A5")]];
    u8 data[length] [[color("FFE156")]];
    padding[-(length % 4) & 3];
};

enum TrackFileType: u32 {
    Normal = 0,
    // i think stream n shit exist
};

struct TrackNumerics {
    padding[88 - 16];
    TrackFileType file_type;
    u16 file_folder_count;
    u16 library_folder_count; // ?
    padding[12];
    u32 bitrate_kbps;
    MacTime date_added;
    padding[148 -(112 + 4)];
    MacTime date_modified;
    u32 normalization; // ?
    padding[176 - (152 + 4)];
    u32 length_ms;
    u32 cloud_catalog_album_id;
    padding[4];
    u32 cloud_catalog_artist_id;
    padding[316 - (176 + 16)];
    u32 file_size;
    padding[4];
    u32 cloud_catalog_track_id;
    padding[12];
    u32 cloud_catalog_track_id_again;
    padding[16];
    u32 cloud_catalog_track_id_again_again;
    padding[4];
    u32 cloud_catalog_track_id_again_again_again;
};

struct TrackPlayStatistics {
    padding[4]; // pad
    padding[8]; // persistent id
    MacTime last_played_at;
    u32 times_played;
};

struct CollectionMember {
    type::Magic<"ipfa"> signature;
    u32 length;
    padding[12];
    PersistentID track_id;
    padding[length - rel];   
};

struct BomaUTF16Content {
    padding[8];
    u32 str_len_bytes [[color("6C5B7F")]];
    padding[8];
    char16 value[str_len_bytes / 2] [[color("FF6F61")]];
};

struct Boma {
    type::Magic<"boma"> signature [[color("92A8D1")]];
    type::Magic<"\x14\x00\x00\x00"> [[hidden]];
    u32 length [[color("F6D55C")]];
    BomaSubtype type [[color("B565A7")]];
                        
    match (type) {
        (
            BomaSubtype::TrackTitle |
            BomaSubtype::Album |
            BomaSubtype::Artist |
            BomaSubtype::Genre |
            BomaSubtype::Kind |
            BomaSubtype::Comment |
            BomaSubtype::Composer |
            BomaSubtype::Grouping |
            BomaSubtype::AlbumArtist |
            BomaSubtype::ClassicalWorkName |
            BomaSubtype::ClassicalMovementTitle |
            BomaSubtype::FairPlayInfo |
            BomaSubtype::SortOrderTrackTitle |
            BomaSubtype::SortOrderAlbum |
            BomaSubtype::SortOrderArtist |
            BomaSubtype::SortOrderAlbumArtist |
            BomaSubtype::SortOrderComposer |
            BomaSubtype::CopyrightHolder |
            BomaSubtype::TrackLocalFilePath |
            BomaSubtype::PurchaserEmail |
            BomaSubtype::PurchaserName |
            BomaSubtype::PlaylistName |
            BomaSubtype::IamaAlbum |
            BomaSubtype::IamaAlbumArtist |
            BomaSubtype::IamaAlbumArtistCloud |
            BomaSubtype::SeriesTitle |
            BomaSubtype::IamaAlbumCloudId |
            BomaSubtype::ArtistsArtistName |
            BomaSubtype::ArtistsArtistNameSorted |
            BomaSubtype::ArtistsArtistCloudId |
            BomaSubtype::AccountCloudId |
            BomaSubtype::AccountDisplayName |
            BomaSubtype::AccountUsername |
            BomaSubtype::AccountUrlSafeId |
            BomaSubtype::AccountAvatarUrl |
            BomaSubtype::UnknownHex1 |
            BomaSubtype::ManagedMediaFolder |
            BomaSubtype::UnknownHex2
        ): BomaUTF16Content utf16 [[inline]];
        (
            BomaSubtype::PlistTrackCloudInformation |
            BomaSubtype::PlistCloudDownloadInformation |
            BomaSubtype::PlistArtworkURL |
            BomaSubtype::PlistPlaylistInfo |
            BomaSubtype::TrackLocalFilePathUrl
        ): {
            padding[4];
            if (header.format_major < 20 && type == BomaSubtype::TrackLocalFilePathUrl) {
                padding[16];
            }
            char value[length - rel];
        }
        (
           BomaSubtype::BookVariant0 |
           BomaSubtype::BookVariant2 |
           BomaSubtype::BookVariant2 |
           BomaSubtype::BookVariant3
        ): {
            padding[4];
            if (header.format_major >= 20) {
                // wow that's just awesome
                char16 value[(length - rel) / 2];
            } else {
                type::Magic<"book"> signature [[color("FF6F61")]];
                padding[48];
                BomaBookMember members[while($ != addressof(this) + length)];
            }
        }
        (BomaSubtype::TrackNumerics): {
            TrackNumerics numerics;
            padding[length - rel];
        }
        (BomaSubtype::TrackPlayStatistics): {
            TrackPlayStatistics played;
            padding[length - rel];
        }
        (BomaSubtype::CollectionMember): {
            padding[4];
            CollectionMember details;
        }
        (_): padding[length - rel];
    }
};

struct SkippedSingular {
    char signature[4] [[static]];
    u32 length;
    padding[length - rel];
};
struct SkippedWithFollowing {
    char signature[4] [[static]];
    u32 length;
    u32 length_full;
    padding[length - rel];
    padding[length_full - length];
};

using LibraryMaster = GenericList<type::Magic<"plma">, Boma, false, SkippedWithFollowing>;
LibraryMaster master @ $;
SectionBoundary bound2 @ $;

struct SimpleBomaContainer<T, auto P> {
    T signature [[static]];
    u32 length;
    u32 length_w_boma;
    u32 boma_count;
    PersistentID id;
    padding[length - rel];
    
    if (P) {
        Boma bomas[boma_count];
    } else {
        padding[length_w_boma - length];
    }
};

using Album = SimpleBomaContainer<type::Magic<"iama">, process_album_boma>;
using AlbumList = GenericList<type::Magic<"lama">, Album, !process_album, SkippedWithFollowing>;
AlbumList albums @ $;
SectionBoundary bound3 @ $;


struct Artist {
    type::Magic<"iAma"> signature [[static]];
    u32 length;
    u32 length_w_boma;
    u32 boma_count;
    PersistentID id;
    padding[28];
    u32 cloud_catalog_id;
    padding[length - rel];
    
    if (process_artist_boma) {
        Boma bomas[boma_count];
    } else {
        padding[length_w_boma - length];
    }
};

using ArtistList = GenericList<type::Magic<"lAma">, Artist, !process_artist, SkippedWithFollowing>;
ArtistList artists @ $;
SectionBoundary bound4 @ $;;


using Account = SimpleBomaContainer<type::Magic<"isma">, process_account_boma>;
using AccountList = GenericList<type::Magic<"Lsma">, Account, !process_account, SkippedWithFollowing>;
char peek_account_signature[4] @ $ [[no_unique_address, hidden]];
struct MaybeAccountList {
    if (peek_account_signature == "Lsma" ) {
        AccountList list [[inline]];
        SectionBoundary bound;
    } else {}
};

MaybeAccountList accounts @ $;

enum Opinion: u16 {
    None = 0,
    Loved = 2,
    Dislike = 3
};

struct Track {
    type::Magic<"itma"> signature;
    u32 length;
    u32 length_w_boma;
    u32 boma_count;
    PersistentID id;
    padding[18];
    bool checked; padding[1];
    padding[18];
    Opinion opinion;
    padding[4];
    u8 stars; padding[3];
    padding[18];
    u16 movements;
    u16 movement;
    padding[74];
    u32 track_release_year;
    PersistentID albumID;
    PersistentID artistID;
    padding[length - rel];
    
    if (process_track_boma) {
        Boma bomas[boma_count];
    } else {
        padding[length_w_boma - length];
    }
};

using TrackList = GenericList<type::Magic<"ltma">, Track, !process_track, SkippedWithFollowing>;
TrackList tracks @ $;
SectionBoundary bound5 @ $;

struct Collection {
    type::Magic<"lpma"> signature;
    u32 length;
    u32 length_w_boma;
    u32 boma_count;
    u32 track_count;
    padding[2];
    MacTime creation_date;
    padding[4];
    PersistentID id;
    padding[41];
    u8 preset_kind;
    padding[100 - 42];
    MacTime modification_date;
    
    padding[length - rel];
    if (process_collection_boma) {
        Boma bomas[boma_count];
    } else {
        padding[length_w_boma - length];
    }
};



using CollectionList = GenericList<type::Magic<"lPma">, Collection, !process_collection, SkippedWithFollowing>;
CollectionList collections @ $;
//SectionBoundary bound6 @ $;;

