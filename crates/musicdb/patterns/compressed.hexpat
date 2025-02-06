#include <std/mem.pat>

#pragma magic 68 66 6D 61 @ 0 // hfma

enum MusicDbFile: u32 {
    LibraryPreferences = 2,
    Application_Catalina = 4,
    Application_BigSur = 5,
    Library_Legacy = 6,
    Library_Sonoma = 7,
};

using PersistentID = u64; // Persistent ID
using MacTime = u32; // todo pls explain

struct Envelope {
    char signature[4];
    u32 header_size;
    u32 file_size;
    u16 format_major;
    u16 format_minor;
    char version[];
    padding[32 - sizeof(version)]; // trailing null
    PersistentID library_persistent_id;
    MusicDbFile file;
    u32 v2;
    u32 v3;
    u32 v4;
    u32 v5;
    u32 v6;
    u32 v7;
    u32 max_crypt_size;
    s32 time_offset; // in seconds
    u32 apple_store_id; // ?
    u32 v10;
    u32 v11;
    u32 v12;
    padding[8];
    u32 v13;
    u64 v14;
    padding[32];
};

Envelope header @ 0;

