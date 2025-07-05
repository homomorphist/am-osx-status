PRAGMA foreign_keys = OFF;
PRAGMA journal_mode = DELETE;
PRAGMA synchronous = OFF;
DROP TABLE IF EXISTS pending_dispatches;
DROP TABLE IF EXISTS errors;
DROP TABLE IF EXISTS deferred_tracks;
DROP TABLE IF EXISTS sessions;
DROP TABLE IF EXISTS custom_artwork_urls;
VACUUM;
