CREATE VIEW IF NOT EXISTS albums (
    title,
    artist,
    format
) AS SELECT DISTINCT album, artist, extension FROM tracks;
