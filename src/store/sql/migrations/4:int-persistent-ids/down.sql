PRAGMA foreign_keys = OFF;
BEGIN TRANSACTION;

-- we're going downwards so old is desired
CREATE TABLE deferred_tracks_old (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    title          TEXT NOT NULL,
    artist         TEXT,
    album          TEXT,
    album_artist   TEXT,
    album_index    INTEGER,
    persistent_id  TEXT NOT NULL,
    duration       REAL NOT NULL,
    media_kind     TEXT NOT NULL
) STRICT;

INSERT INTO deferred_tracks_old(id, title, artist, album, album_artist, album_index, persistent_id, duration, media_kind)
    SELECT id, title, artist, album, album_artist, album_index, CAST(persistent_id AS TEXT), duration, media_kind
    FROM deferred_tracks;

DROP TABLE deferred_tracks;
ALTER TABLE deferred_tracks_old RENAME TO deferred_tracks;

COMMIT;
PRAGMA foreign_keys = ON;
PRAGMA foreign_key_check;
