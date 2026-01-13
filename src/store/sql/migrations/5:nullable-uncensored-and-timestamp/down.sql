BEGIN TRANSACTION;

CREATE TABLE uncensored_titles_old ( 
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    persistent_id  INTEGER NOT NULL,
    uncensored        TEXT NOT NULL
);

INSERT INTO uncensored_titles_old SELECT id, persistent_id, uncensored FROM uncensored_titles WHERE uncensored IS NOT NULL;
DROP TABLE uncensored_titles;
ALTER TABLE uncensored_titles_old RENAME TO uncensored_titles;

COMMIT;
