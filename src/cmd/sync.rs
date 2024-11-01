use crate::cmd::*;
use crate::db;
use crate::model;
use anyhow::anyhow;
use anyhow::Ok;
use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use fs_extra::file::{copy_with_progress, CopyOptions};
use indicatif::MultiProgress;
use std::collections::hash_set;
use std::os::unix::fs::MetadataExt;

#[derive(ClapArgs)]
pub struct Args {
    /// Path where to look for tracksync source data.
    #[arg(short, long, default_value_t = db::default_database_dir().to_str().unwrap().to_owned())]
    pub database_path: String,

    /// Path where to store tracksync's database, as well as music files.
    #[arg(long)]
    pub destination: Option<String>,

    /// Do not delete from destination tracks that are not contained in the local database instance.
    #[arg(long, default_value_t = false)]
    pub no_delete: bool,

    /// Do not attempt to copy any file or write any change on destination database, just
    /// print what tracks would be copied over.
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,

    /// Instead of copying the files over to the specified destination, create an hardlink.
    #[arg(long, default_value_t = false)]
    pub link: bool,
}

impl Args {
    pub fn validate(&self) -> Result<()> {
        if let None = self.destination {
            return Err(anyhow!(error::Error::ValidationError(
                "missing destination".to_owned(),
            )));
        };

        Ok(())
    }
}

pub async fn run(args: Args) -> Result<()> {
    args.validate()?;

    let dest_dir = args.destination.unwrap();

    let local_db = db::Instance::new(&args.database_path, false)
        .await
        .with_context(|| "Cannot open local database instance")?;

    let dest_db = db::Instance::new(&dest_dir, true)
        .await
        .with_context(|| "Cannot open destination database instance")?;

    let diff = db::diff(&local_db, &dest_db)
        .await
        .with_context(|| "Cannot calculate difference between local and destination databases")?;

    let mut reverse_diff = db::diff(&dest_db, &local_db)
        .await
        .with_context(|| "Cannot calculate difference between destination and local databases")?;

    let raw_filter = local_db
        .filter()
        .await
        .with_context(|| "Could not fetch filters.")?;

    let filters = match raw_filter {
        Some(raw_filter) => Some(
            crate::filter::evaluate(vec![raw_filter])
                .with_context(|| "Could not evaluate filters")?,
        ),
        None => None,
    };

    // find any filtered tracks that were already copied
    reverse_diff.append(&mut diff_databases(&local_db, &dest_db, filters.as_ref(), false).await?);

    // now filter out all tracks to copy by using the filters
    let diff = filter_tracks_by_id(filters.as_ref(), &local_db, diff).await?;

    if args.dry_run {
        dry_run_copy(&local_db, &dest_dir, diff, filters.as_ref()).await?;
        dry_run_delete(&dest_db, &dest_dir, reverse_diff, filters.as_ref()).await?;
        return Ok(());
    }

    if !args.no_delete {
        run_delete(&dest_db, &dest_dir, reverse_diff, filters.as_ref()).await?
    }

    run_copy(
        &local_db,
        &dest_db,
        &dest_dir,
        diff,
        filters.as_ref(),
        args.link,
    )
    .await?;

    Ok(())
}

fn progress_bar(size: u64, style: indicatif::ProgressStyle) -> indicatif::ProgressBar {
    let bar = indicatif::ProgressBar::new(size);
    bar.set_style(style);

    bar
}

fn total_style() -> indicatif::ProgressStyle {
    indicatif::ProgressStyle::with_template(
        "Total progress:\n[{percent}% {wide_bar:.green}] {human_pos}/{human_len} {elapsed}\n\n",
    )
    .unwrap()
    .progress_chars("##-")
}

fn delete_style() -> indicatif::ProgressStyle {
    indicatif::ProgressStyle::with_template(
        "Deleting old tracks:\n[{percent}% {wide_bar:.green}] {human_pos}/{human_len} {elapsed}\n\n",
    )
    .unwrap()
    .progress_chars("##-")
}

fn track_style() -> indicatif::ProgressStyle {
    indicatif::ProgressStyle::with_template(
        "{msg}\n[{percent}% {wide_bar:.green}] {decimal_bytes:>7}/{decimal_total_bytes:7} {elapsed}\n\n",
    )
    .unwrap()
    .progress_chars("##-")
}

async fn diff_databases(
    source: &db::Instance,
    destination: &db::Instance,
    filters: Option<&Vec<crate::filter::ScriptRuntime>>,
    delete: bool,
) -> Result<Vec<String>> {
    let local_tracks = source.tracks_by_state(model::FileState::Copied).await?;

    let local_tracks = filter_tracks(local_tracks, filters, delete)?;

    let dest_tracks = destination
        .tracks_by_state(model::FileState::Copied)
        .await?;

    let src_ids: hash_set::HashSet<String> = local_tracks
        .clone()
        .into_iter()
        .map(|t| t.track_id)
        .collect();

    let dst_ids: hash_set::HashSet<String> = dest_tracks
        .clone()
        .into_iter()
        .map(|t| t.track_id)
        .collect();

    log::debug!("local {} dest {}", src_ids.len(), dst_ids.len());

    let d: hash_set::HashSet<&String> = dst_ids.difference(&src_ids).collect();

    Ok(d.into_iter().map(|e| e.clone()).collect())
}

async fn filter_tracks_by_id(
    filters: Option<&Vec<crate::filter::ScriptRuntime>>,
    db: &db::Instance,
    ids: Vec<String>,
) -> Result<Vec<String>> {
    let tracks = db.tracks_by_id(ids).await?;

    let tracks = filter_tracks(tracks, filters, false)?;

    Ok(tracks.into_iter().map(|t| t.track_id).collect())
}

fn filter_tracks(
    raw_tracks: Vec<model::Track>,
    filters: Option<&Vec<crate::filter::ScriptRuntime>>,
    delete: bool,
) -> Result<Vec<model::Track>> {
    if let Some(filters) = filters {
        let mut filter_res = vec![];

        for f in filters {
            let base_tracks = raw_tracks
                .clone()
                .into_iter()
                .map(|t| Into::<model::BaseTrack>::into(t))
                .collect();

            filter_res = f.run(base_tracks)?;
        }

        return Ok(raw_tracks
            .into_iter()
            .enumerate()
            .filter_map(|elem| {
                let (idx, track) = elem;

                if filter_res[idx] && !delete {
                    return None;
                }

                return Some(track);
            })
            .collect());
    }

    Ok(raw_tracks)
}

async fn run_copy(
    local_db: &db::Instance,
    dest_db: &db::Instance,
    dest_dir: &String,
    diff: Vec<String>,
    filters: Option<&Vec<crate::filter::ScriptRuntime>>,
    link: bool,
) -> Result<()> {
    let diff_len = diff.len();

    let tracks = filter_tracks(
        local_db
            .tracks_by_id(diff)
            .await
            .with_context(|| "Cannot get tracks from local database")?,
        filters,
        false,
    )?;

    // Copy tracks
    let mp = MultiProgress::new();

    let total_bar = mp.add(progress_bar(diff_len as u64, total_style()));

    total_bar.tick();

    for track in tracks {
        copy(track, &dest_db, &dest_dir, &mp, link).await?;
        total_bar.inc(1);
    }

    total_bar.finish();

    Ok(())
}

async fn dry_run_copy(
    local_db: &db::Instance,
    dest_dir: &String,
    diff: Vec<String>,
    filters: Option<&Vec<crate::filter::ScriptRuntime>>,
) -> Result<()> {
    let tracks = filter_tracks(
        local_db
            .tracks_by_id(diff)
            .await
            .with_context(|| "Cannot get tracks from local database")?,
        filters,
        false,
    )?;

    for track in tracks {
        let track_storage_path = track.storage_path(&dest_dir);

        log::info!("Will copy {} to {}", track.file_path, track_storage_path);
    }

    Ok(())
}

async fn dry_run_delete(
    dest_db: &db::Instance,
    dest_dir: &String,
    diff: Vec<String>,
    filters: Option<&Vec<crate::filter::ScriptRuntime>>,
) -> Result<()> {
    let tracks = filter_tracks(
        dest_db
            .tracks_by_id(diff)
            .await
            .with_context(|| "Cannot get tracks from destination database")?,
        filters,
        true,
    )?;

    for track in tracks {
        let track_storage_path = track.storage_path(&dest_dir);

        log::info!("Will delete {}", track_storage_path)
    }

    Ok(())
}

async fn run_delete(
    dest_db: &db::Instance,
    dest_dir: &String,
    diff: Vec<String>,
    filters: Option<&Vec<crate::filter::ScriptRuntime>>,
) -> Result<()> {
    let diff_len = diff.len();

    if diff_len == 0 {
        return Ok(());
    }

    let tracks = filter_tracks(
        dest_db
            .tracks_by_id(diff)
            .await
            .with_context(|| "Cannot get tracks from destination database")?,
        filters,
        true,
    )?;

    let mp = MultiProgress::new();

    let total_bar = mp.add(progress_bar(diff_len as u64, delete_style()));

    total_bar.tick();

    for track in tracks {
        delete(track, &dest_db, &dest_dir, &mp).await?;
        total_bar.inc(1);
    }

    total_bar.finish();

    Ok(())
}

async fn delete(
    track: model::Track,
    dest_db: &db::Instance,
    dest_dir: &String,
    mp: &indicatif::MultiProgress,
) -> Result<()> {
    let track_storage_path = track.storage_path(&dest_dir);

    let bar = mp.add(
        progress_bar(1, track_style()).with_message(format!("Deleting: {}", track_storage_path)),
    );

    dest_db.delete(track.id).await?;
    std::fs::remove_file(track_storage_path.clone())
        .with_context(|| format!("Cannot delete file {}", track_storage_path.clone()))?;

    bar.inc(1);

    mp.remove(&bar);

    Ok(())
}

async fn copy(
    track: model::Track,
    dest_db: &db::Instance,
    dest_dir: &String,
    mp: &indicatif::MultiProgress,
    link: bool,
) -> Result<()> {
    let track_storage_path = track.storage_path(&dest_dir);
    let sp = std::path::Path::new(&track_storage_path);

    let parent = sp
        .parent()
        .with_context(|| "Cannot obtain base destination directory")?;

    // step 1: add an in-flight copy to the destination database
    let mut dest_track = track.clone();
    dest_track.file_state = crate::model::FileState::Copying;
    dest_db
        .insert_track(&dest_track)
        .await
        .with_context(|| "Cannot insert in-progress copying track in destination database")?;

    // step 2: actually copy the track
    std::fs::create_dir_all(parent).with_context(|| {
        format!(
            "Cannot create destination directory tree {}",
            parent.to_str().unwrap()
        )
    })?;

    let orig_file_path = std::path::Path::new(&track.file_path);
    let orig_file = std::fs::File::open(orig_file_path).with_context(|| {
        format!(
            "Cannot open origin file {}",
            orig_file_path.to_str().unwrap()
        )
    })?;

    let orig_file_meta = orig_file.metadata().with_context(|| {
        format!(
            "Cannot obtain metadata information of {}",
            orig_file_path.to_str().unwrap()
        )
    })?;

    let bar = mp.add(
        progress_bar(orig_file_meta.size(), track_style()).with_message(format!(
            "Copying: {}\nTo: {}",
            track.file_path, track_storage_path
        )),
    );

    let opts = CopyOptions::new().overwrite(true);

    if link {
        std::fs::hard_link(track.file_path.clone(), track_storage_path.clone())?;
    } else {
        match copy_with_progress(
            track.file_path.clone(),
            track_storage_path.clone(),
            &opts,
            |ph| {
                bar.set_position(ph.copied_bytes);
            },
        ) {
            std::result::Result::Ok(_) => {}
            Err(err) => {
                return Err(error::Error::CopyError(err)).with_context(|| {
                    format!("Cannot copy {} to {}", track.file_path, track_storage_path)
                });
            }
        };
        // .with_context(|| format!("Cannot copy {} to {}", track.file_path, track_storage_path))?;
    }

    // step 3: update the destination track with the new state
    dest_track.file_state = crate::model::FileState::Copied;
    dest_db
        .insert_track(&dest_track)
        .await
        .with_context(|| "Cannot insert copy finished track in destination database")?;

    bar.finish();

    mp.remove(&bar);

    Ok(())
}
