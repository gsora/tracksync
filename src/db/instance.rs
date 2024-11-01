use async_std::channel::{Receiver, Sender};
use futures::StreamExt;
use sqlx::{migrate::Migrator, sqlite::SqliteConnectOptions, Error, Row, SqlitePool};

use crate::model;

static MIGRATOR: Migrator = sqlx::migrate!("database/migrations/local");

static DATABASE_DEFAULT_NAME: &str = "tracksync.db";

pub struct Instance {
    pool: SqlitePool,
}

impl Instance {
    pub async fn new(database_path: &str, is_external: bool) -> Result<Instance, sqlx::Error> {
        let db_path = std::path::Path::new(database_path);

        if !db_path.is_dir() {
            return Err(Error::Protocol(format!(
                "{database_path} is not a directory"
            )));
        }

        let mut db_path = db_path.to_path_buf();
        db_path.push(DATABASE_DEFAULT_NAME);

        let db_path = db_path.to_str().unwrap();

        let sqlite_conn_str = ("sqlite:".to_owned()) + db_path;
        log::debug!("database path: {sqlite_conn_str}");

        let pool = SqlitePool::connect_with(
            SqliteConnectOptions::new()
                .create_if_missing(true)
                .filename(db_path),
        )
        .await?;

        MIGRATOR.run(&pool).await?;

        let i = Instance { pool };

        match crate::db::lib::is_initialized(&i.pool).await {
            Ok(_) => (),
            Err(err) => {
                match err {
                    Error::RowNotFound => i.initialize_state(is_external).await?,
                    err => return Err(err),
                };
                ()
            }
        };

        if crate::db::lib::is_external(&i.pool).await? != is_external {
            return Err(Error::Protocol(format!(
                "database is marked as non-external, but it is",
            )));
        }

        Ok(i)
    }

    pub async fn initialize_state(&self, is_external: bool) -> Result<(), Error> {
        let mut conn = self.pool.acquire().await?;

        sqlx::query!(
            r#"
            INSERT INTO state (
                version,
                is_external
            ) VALUES (
                ?1,
                ?2
            );
            "#,
            "1.0",
            is_external,
        )
        .execute(&mut *conn)
        .await?;

        Ok(())
    }

    pub async fn exists(&self, path: String) -> Result<bool, Error> {
        let mut conn = self.pool.acquire().await?;

        match sqlx::query!(
            r#"
            SELECT id FROM tracks WHERE file_path = ?1;
            "#,
            path,
        )
        .fetch_one(&mut *conn)
        .await
        {
            Ok(_) => Ok(true),
            Err(err) => match err {
                Error::RowNotFound => Ok(false),
                rest => Err(rest),
            },
        }
    }

    pub async fn insert_track(&self, track: &model::Track) -> Result<(), Error> {
        let mut conn = self.pool.acquire().await?;

        sqlx::query!(
            r#"
            INSERT OR REPLACE INTO tracks (
                track_id,
                title,
                artist,
                album,
                number,
                file_path,
                disc_number,
                disc_total,
                file_state,
                extension
            ) VALUES (
                ?1,
                ?2,
                ?3,
                ?4,
                ?5,
                ?6,
                ?7,
                ?8,
                ?9,
                ?10
            );
            "#,
            track.track_id,
            track.title,
            track.artist,
            track.album,
            track.number,
            track.file_path,
            track.disc_number,
            track.disc_total,
            track.file_state,
            track.extension,
        )
        .execute(&mut *conn)
        .await?;

        Ok(())
    }

    pub async fn track_ids_by_state(&self, state: model::FileState) -> Result<Vec<String>, Error> {
        let mut conn = self.pool.acquire().await?;

        Ok(sqlx::query!(
            r#"
            SELECT track_id FROM tracks WHERE file_state = ?1;
            "#,
            state,
        )
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|t| t.track_id)
        .collect::<Vec<String>>())
    }

    pub async fn tracks_by_id(&self, ids: Vec<String>) -> Result<Vec<model::Track>, Error> {
        let mut conn = self.pool.acquire().await?;

        let ids_joined = ids
            .into_iter()
            .map(|mut id| {
                id.insert_str(0, "'");
                id.push_str("'");

                id
            })
            .collect::<Vec<_>>()
            .join(",");

        let query = format!("select * from tracks where track_id in ({});", ids_joined);
        let rows = sqlx::query(&query).fetch_all(&mut *conn).await?;

        Ok(rows
            .into_iter()
            .map(|r| model::Track {
                id: r.get("id"),
                track_id: r.get("track_id"),
                title: r.get("title"),
                artist: r.get("artist"),
                album: r.get("album"),
                number: r.get("number"),
                file_path: r.get("file_path"),
                disc_number: r.get("disc_number"),
                disc_total: r.get("disc_total"),
                file_state: r.get("file_state"),
                extension: r.get("extension"),
            })
            .collect())
    }

    pub async fn tracks_by_state(
        &self,
        state: model::FileState,
    ) -> Result<Vec<model::Track>, Error> {
        let mut conn = self.pool.acquire().await?;

        Ok(sqlx::query!(
            r#"
            SELECT * FROM tracks WHERE file_state = ?1;
            "#,
            state,
        )
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|r| model::Track {
            id: r.id,
            track_id: r.track_id,
            title: r.title,
            artist: r.artist,
            album: r.album,
            number: r.number,
            file_path: r.file_path,
            disc_number: r.disc_number,
            disc_total: r.disc_total,
            file_state: r.file_state.into(),
            extension: r.extension,
        })
        .collect::<Vec<model::Track>>())
    }

    pub async fn delete(&self, id: i64) -> Result<(), Error> {
        let mut conn = self.pool.acquire().await?;

        sqlx::query!(
            r#"
            DELETE FROM tracks WHERE id = ?1;
            "#,
            id,
        )
        .execute(&mut *conn)
        .await?;

        Ok(())
    }

    pub async fn directories(&self) -> Result<Vec<String>, Error> {
        let mut conn = self.pool.acquire().await?;

        Ok(sqlx::query!(r#"SELECT * FROM directories;"#)
            .fetch_all(&mut *conn)
            .await?
            .into_iter()
            .map(|e| e.directory)
            .collect())
    }

    pub async fn insert_directory(&self, directory: String) -> Result<(), Error> {
        let mut conn = self.pool.acquire().await?;

        sqlx::query!(
            r#"INSERT OR REPLACE INTO directories (directory) VALUES (?1);"#,
            directory,
        )
        .execute(&mut *conn)
        .await?;

        Ok(())
    }

    pub async fn track_paths_from_dir(&self, directory: String) -> Result<Vec<String>, Error> {
        let mut conn = self.pool.acquire().await?;

        let directory = format!("{}?", directory);
        Ok(sqlx::query!(
            r#"SELECT file_path FROM tracks where file_path LIKE ?1"#,
            directory,
        )
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|e| e.file_path)
        .collect())
    }

    pub async fn albums(&self) -> Result<Vec<model::Album>, Error> {
        let mut conn = self.pool.acquire().await?;

        Ok(sqlx::query!(
            r#"
            SELECT * FROM albums;
            "#
        )
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|e| model::Album {
            title: e.title,
            artist: e.artist,
            format: e.format,
        })
        .collect())
    }

    pub async fn fuzzy_find_album(
        &self,
        query: &Vec<String>,
    ) -> Result<Vec<(String, String, String)>, Error> {
        let mut conn = self.pool.acquire().await?;

        let query_str = format!(
            r#"select track_id, album, extension from track_fts where album match '{}' group by album;"#,
            query.join(" "),
        );

        Ok(sqlx::query(&query_str)
            .fetch_all(&mut *conn)
            .await?
            .into_iter()
            .map(|r| {
                let album: String = r.get("album");
                let format: String = r.get("extension");
                let track_id: String = r.get("track_id");

                (track_id, album, format)
            })
            .collect())
    }

    pub async fn duplicate_albums(&self) -> Result<Vec<(model::Album, i64)>, Error> {
        let mut conn = self.pool.acquire().await?;

        Ok(sqlx::query!(
            r#"
                SELECT artist, title, count(*) as count FROM albums
                GROUP BY artist, title
                HAVING count > 1;
            "#,
        )
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|e| {
            (
                model::Album {
                    title: e.title,
                    artist: e.artist,
                    format: String::new(),
                },
                e.count,
            )
        })
        .collect())
    }

    pub async fn album_paths(
        &self,
        title: &String,
        artist: &String,
    ) -> Result<Vec<(String, String)>, Error> {
        let mut conn = self.pool.acquire().await?;

        Ok(sqlx::query!(
            r#"
                select * from tracks where artist = ?2 and album = ?1 group by extension;
            "#,
            title,
            artist,
        )
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|e| {
            // parse dupe path and get the directory containing it
            let dupe_path = std::path::Path::new(&e.file_path).parent().unwrap();
            let dupe_path = dupe_path.to_str().unwrap().to_string();

            (dupe_path.clone(), e.extension.clone())
        })
        .collect())
    }

    pub async fn tracks_iter(&self) -> Result<Receiver<Result<model::Track, Error>>, Error> {
        let mut conn = self.pool.acquire().await?;

        let (tx, rx): (
            Sender<Result<model::Track, Error>>,
            Receiver<Result<model::Track, Error>>,
        ) = async_std::channel::unbounded();

        async_std::task::spawn(async move {
            let mut tracks_stream = sqlx::query!("SELECT * from tracks;").fetch(&mut *conn);

            while let Some(track) = tracks_stream.next().await {
                match track {
                    Ok(track) => tx
                        .send(Ok(model::Track {
                            id: track.id,
                            track_id: track.track_id,
                            title: track.title,
                            artist: track.artist,
                            album: track.album,
                            number: track.number,
                            file_path: track.file_path,
                            disc_number: track.disc_number,
                            disc_total: track.disc_total,
                            file_state: track.file_state.into(),
                            extension: track.extension,
                        }))
                        .await
                        .unwrap(),
                    Err(e) => {
                        tx.send(Err(e)).await.unwrap();
                        tx.close();
                        break;
                    }
                }
            }

            tx.close();
        });

        Ok(rx)
    }

    pub async fn filter(&self) -> Result<Option<String>, Error> {
        let mut conn = self.pool.acquire().await?;

        Ok(sqlx::query!(
            r#"
                select filter from state;
            "#,
        )
        .fetch_one(&mut *conn)
        .await?
        .filter)
    }

    pub async fn set_filter(&self, filter: String) -> Result<(), Error> {
        let mut conn = self.pool.acquire().await?;

        sqlx::query!(
            r#"
            update state set filter = ?1;"#,
            filter,
        )
        .execute(&mut *conn)
        .await?;

        Ok(())
    }
}
