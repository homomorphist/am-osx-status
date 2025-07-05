-- SQLITE
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;

CREATE TABLE IF NOT EXISTS sessions (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    ver_crate  TEXT NOT NULL, -- semver of this crate
    ver_player TEXT NOT NULL, -- semver-ish (has four parts for Apple Music, iTunes known) version of the player
    ver_os     TEXT,          -- semver of the operating system. null if undetermined
    osa_fetches_track   INTEGER NOT NULL DEFAULT(0), -- # jxa fetches for current track
    osa_fetches_player  INTEGER NOT NULL DEFAULT(0), -- # jxa fetches for player/app status
    started_at  INTEGER NOT NULL DEFAULT(unixepoch('subsec') * 1000), 
      ended_at  INTEGER
) STRICT;

CREATE TABLE IF NOT EXISTS deferred_tracks (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    title          TEXT NOT NULL,
    artist         TEXT,
    album          TEXT,
    album_artist   TEXT,
    album_index    INTEGER, -- 1-based; "track number"
    persistent_id  TEXT NOT NULL,
    duration       REAL NOT NULL, -- in seconds
    media_kind     TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS errors (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp    INTEGER NOT NULL DEFAULT(unixepoch('subsec') * 1000),
    session      INTEGER NOT NULL,
    fmt_display  TEXT NOT NULL,
    fmt_debug    TEXT NOT NULL,
    FOREIGN KEY(session) REFERENCES sessions(id)
) STRICT;

CREATE TABLE IF NOT EXISTS pending_dispatches (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp    INTEGER NOT NULL DEFAULT(unixepoch('subsec') * 1000),
    backend      TEXT,
    track        INTEGER NOT NULL,
    error        INTEGER NOT NULL,
    FOREIGN KEY(track) REFERENCES deferred_tracks(id),
    FOREIGN KEY(error) REFERENCES errors(id)
) STRICT;

CREATE TABLE IF NOT EXISTS custom_artwork_urls (
    id           INTEGER PRIMARY KEY AUTOINCREMENT, 
    source_path  TEXT    NOT NULL, -- path to the file on disk
    uploaded_at  INTEGER NOT NULL DEFAULT(unixepoch('subsec') * 1000),
     expires_at  INTEGER, -- unix epoch, milliseconds
    artwork_url  TEXT    NOT NULL
) STRICT;
