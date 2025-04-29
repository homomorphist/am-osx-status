PRAGMA foreign_keys = OFF;
PRAGMA journal_mode = DELETE;
PRAGMA synchronous = OFF;
DROP TABLE IF EXISTS pending_dispatch;
DROP TABLE IF EXISTS errors;
DROP TABLE IF EXISTS deferred_track;
DROP TABLE IF EXISTS session;
VACUUM;
