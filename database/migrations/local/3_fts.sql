CREATE VIRTUAL TABLE track_fts USING fts5(track_id, title, album, artist, extension, content=tracks, content_rowid=id);

CREATE TRIGGER track_fts_ai_insert AFTER INSERT ON tracks BEGIN
  INSERT INTO track_fts(rowid, track_id, title, album, artist, extension) VALUES (new.id, new.track_id, new.title, new.album, new.artist, new.extension);
END;

CREATE TRIGGER track_fts_ai_delete AFTER DELETE ON tracks BEGIN
  INSERT INTO track_fts(track_fts, rowid, track_id, title, album, artist, extension) VALUES('delete', old.id, old.track_id, old.title, old.album, old.artist, old.extension);
END;
