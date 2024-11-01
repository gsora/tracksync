use std::{collections::hash_set, path::PathBuf};

use sqlx::{Error, SqlitePool};

use super::instance::Instance;

pub(crate) async fn is_initialized(pool: &SqlitePool) -> Result<bool, Error> {
    let mut conn = pool.acquire().await?;

    let res = sqlx::query!(
        r#"
        SELECT version FROM state GROUP BY version;
        "#
    )
    .fetch_one(&mut *conn)
    .await?;

    Ok(res.version == 0)
}

pub(crate) async fn is_external(pool: &SqlitePool) -> Result<bool, Error> {
    let mut conn = pool.acquire().await?;

    let res = sqlx::query!(
        r#"
        SELECT is_external FROM state GROUP BY version;
        "#
    )
    .fetch_one(&mut *conn)
    .await?;

    Ok(res.is_external.unwrap_or_default())
}

pub async fn diff(source: &Instance, destination: &Instance) -> Result<Vec<String>, Error> {
    let source_ids = source
        .track_ids_by_state(crate::model::FileState::Copied)
        .await?;
    log::debug!("source ids: {}", source_ids.len(),);

    let source_ids: hash_set::HashSet<String> = source_ids.into_iter().collect();
    let dest_ids: hash_set::HashSet<String> = destination
        .track_ids_by_state(crate::model::FileState::Copied)
        .await?
        .into_iter()
        .collect();

    log::debug!(
        "source ids: {} dest ids: {}",
        source_ids.len(),
        dest_ids.len()
    );

    let d: hash_set::HashSet<&String> = source_ids.difference(&dest_ids).collect();

    Ok(d.into_iter().map(|e| e.clone()).collect())
}

pub fn default_database_dir() -> PathBuf {
    let bd = directories::BaseDirs::new().unwrap();
    let conf_dir = bd.config_dir();

    conf_dir.to_path_buf()
}
