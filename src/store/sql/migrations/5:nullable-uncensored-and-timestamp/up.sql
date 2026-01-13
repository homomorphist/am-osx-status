BEGIN TRANSACTION;

CREATE TABLE uncensored_titles_new ( 
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    persistent_id  INTEGER NOT NULL,
    uncensored     TEXT,
    timestamp      INTEGER NOT NULL DEFAULT (strftime('%s','now') * 1000)
);

INSERT INTO uncensored_titles_new (id, persistent_id, uncensored) SELECT id, persistent_id, uncensored FROM uncensored_titles;
DROP TABLE uncensored_titles;
ALTER TABLE uncensored_titles_new RENAME TO uncensored_titles;

COMMIT;
