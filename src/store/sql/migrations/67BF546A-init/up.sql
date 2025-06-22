-- SQLITE
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;

CREATE TABLE IF NOT EXISTS session (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    ver_crate  TEXT NOT NULL, -- semver of this crate
    ver_music  TEXT NOT NULL, -- semver-ish (has four parts) of the Apple Music application
    ver_os     TEXT NOT NULL, -- semver of the operating system
                polls   INTEGER NOT NULL DEFAULT(0),
    osa_fetches_track   INTEGER NOT NULL DEFAULT(0), -- # jxa fetches for current track
    osa_fetches_player  INTEGER NOT NULL DEFAULT(0), -- # jxa fetches for player/app status
    started_at  INTEGER NOT NULL DEFAULT(unixepoch('subsec') * 1000), 
      ended_at  INTEGER
) STRICT;

CREATE TABLE IF NOT EXISTS deferred_track (
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
    FOREIGN KEY(session) REFERENCES session(id)
) STRICT;

CREATE TABLE IF NOT EXISTS pending_dispatch (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp    INTEGER NOT NULL DEFAULT(unixepoch('subsec') * 1000),
    backend      TEXT,
    track        INTEGER NOT NULL,
    error        INTEGER NOT NULL,
    FOREIGN KEY(track) REFERENCES deferred_track(id),
    FOREIGN KEY(error) REFERENCES errors(id)
) STRICT;
