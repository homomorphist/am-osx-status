-- ok this thing wasn't even in use yet
-- and despite me saying it's hex, i used actual literals in seeding, so...
-- time to just roll with ints
PRAGMA foreign_keys = OFF;
BEGIN TRANSACTION;

CREATE TABLE deferred_tracks_new (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    title          TEXT NOT NULL,
    artist         TEXT,
    album          TEXT,
    album_artist   TEXT,
    album_index    INTEGER,
    persistent_id  INTEGER NOT NULL,
    duration          REAL NOT NULL,
    media_kind        TEXT NOT NULL
) STRICT;

INSERT INTO deferred_tracks_new(id, title, artist, album, album_artist, album_index, persistent_id, duration, media_kind)
    SELECT id, title, artist, album, album_artist, album_index, CAST(persistent_id AS INTEGER), duration, media_kind
    FROM deferred_tracks;

DROP TABLE deferred_tracks;
ALTER TABLE deferred_tracks_new RENAME TO deferred_tracks;

COMMIT;
PRAGMA foreign_keys = ON;
PRAGMA foreign_key_check;
