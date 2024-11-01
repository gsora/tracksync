CREATE TABLE IF NOT EXISTS state (
    id          INTEGER PRIMARY KEY NOT NULL,
    version     INTEGER NOT NULL,
    is_external BOOL
);

CREATE TABLE IF NOT EXISTS tracks (
    id INTEGER PRIMARY KEY NOT NULL,
    track_id TEXT NOT NULL,
    title TEXT NOT NULL,
    artist TEXT NOT NULL,
    album TEXT NOT NULL,
    number INTEGER NOT NULL,
    disc_number INTEGER NOT NULL,
    disc_total INTEGER NOT NULL,
    file_state INTEGER NOT NULL,
    file_path TEXT NOT NULL,
    extension TEXT NOT NULL
);

CREATE VIEW IF NOT EXISTS albums (
    title,
    artist,
    format
) AS SELECT DISTINCT album, artist, extension FROM tracks;
