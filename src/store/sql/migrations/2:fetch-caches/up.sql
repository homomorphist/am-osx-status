CREATE TABLE IF NOT EXISTS first_artists (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    persistent_id  INTEGER NOT NULL,
    artists           TEXT NOT NULL, -- all artists verbatim. if this doesn't match, song metadata changed so we should recompute
    artist            TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS uncensored_titles (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    persistent_id  INTEGER NOT NULL,
    uncensored        TEXT NOT NULl
) STRICT;
