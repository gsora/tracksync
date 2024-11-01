use std::collections::hash_set;

use crate::cmd::*;
use crate::*;
use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use futures::{executor::block_on, future::try_join_all};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use model::FileState;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Directory in which tunesdirector will store its local database.
    #[arg(short, long, default_value_t = db::default_database_dir().to_str().unwrap().to_owned())]
    pub database_path: String,

    /// A path in which tunesdirector will look for music files.
    /// Specify more than one for multiple sources.
    #[arg(short, long = "source", value_name = "SOURCE", action = clap::ArgAction::Append)]
    pub sources: Option<Vec<String>>,

    /// Specifies if the database is to be written is a destination one.
    #[arg(
        long = "destination",
        value_name = "TRUE|FALSE",
        default_value_t = false
    )]
    pub is_destination: bool,
}

impl Args {
    pub fn validate(&self) -> Result<(), error::Error> {
        if let None = self.sources {
            return Err(error::Error::ValidationError(
                "missing source(s)".to_owned(),
            ));
        };

        Ok(())
    }
}

pub async fn run(args: Args, update: bool) -> Result<()> {
    let val_res = args.validate();

    match update {
        true => {}
        false => val_res?,
    };

    log::debug!("CLI args: {:?}", args);

    // Open a database
    let db = db::Instance::new(&args.database_path, args.is_destination)
        .await
        .with_context(|| "Cannot open local database instance")?;

    let sources = match update {
        false => args.sources.unwrap(),
        true => db
            .directories()
            .await
            .with_context(|| "Cannot fetch track directories from database")?,
    };

    let mp = MultiProgress::new();
    let mut tracks = vec![];

    for source in &sources {
        for i in db
            .track_paths_from_dir(source.clone())
            .await
            .with_context(|| "Cannot fetch track paths from directory")?
        {
            tracks.push(i)
        }
    }

    let tracks_set: hash_set::HashSet<String> = tracks.into_iter().collect();

    let res = try_join_all(
        sources
            .into_iter()
            .map(|source| {
                traverse_and_add_param(&db, &mp, source, {
                    let tracks_set = tracks_set.clone();

                    move |path, db, pb| match update {
                        false => add_dupe_checker(path, db, pb),
                        true => Ok(!tracks_set.contains(path)),
                    }
                })
            })
            .collect::<Vec<_>>(),
    )
    .await?;

    let totals = res.iter().fold((0, 0), |acc, r| (acc.0 + r.0, acc.1 + r.1));

    match totals.1 {
        0 => log::info!("Imported {} tracks", totals.0),
        _ => match update {
            false => log::info!(
                "Imported {} new tracks, but found {} duplicates",
                totals.0,
                totals.1
            ),
            true => {}
        },
    };

    if update {
        let prog = mp.add(
            ProgressBar::new_spinner()
                .with_message("Looking for tracks not on disk anymore...")
                .with_style(ProgressStyle::default_spinner()),
        );

        prog.enable_steady_tick(std::time::Duration::from_millis(50));

        // look for files in db that are not on the filesystem anymore
        let track_iter = db
            .tracks_iter()
            .await
            .with_context(|| "Cannot create an iterator for existing tracks in database")?;

        while let Ok(track) = track_iter.recv().await {
            let track = track?;

            let tp = std::path::Path::new(&track.file_path);

            match tp.exists() {
                true => {}
                false => {
                    prog.set_message(format!(
                        "Found track in database not existing on filesystem, deleting: {}",
                        track.file_path,
                    ));

                    db.delete(track.id)
                        .await
                        .with_context(|| "Cannot delete track from database.")?;
                }
            }
        }

        prog.finish();
        mp.remove(&prog);
    }

    Ok(())
}

fn add_dupe_checker(path: &String, db: &db::Instance, pb: &indicatif::ProgressBar) -> Result<bool> {
    block_on(async {
        if db.exists(path.clone()).await? {
            pb.set_message(format!("Found duplicate at {}", path.clone()));
            return Ok(true);
        }

        return Ok(false);
    })
}

pub(crate) async fn traverse_and_add_param<F>(
    db: &db::Instance,
    mp: &MultiProgress,
    path: String,
    dupe_checker: F,
) -> Result<(u64, u64)>
where
    F: FnOnce(&String, &db::Instance, &indicatif::ProgressBar) -> Result<bool> + Clone,
{
    let paths = fs::traverse(&path).await;

    let base_msg = format!("Reading {}...", path.clone());

    let prog = mp.add(
        ProgressBar::new_spinner()
            .with_message(base_msg.clone())
            .with_style(ProgressStyle::default_spinner()),
    );

    prog.enable_steady_tick(std::time::Duration::from_millis(50));

    let mut new_tracks = 0;
    let mut duplicate = 0;

    while let Ok(p) = paths.recv().await {
        let p = p?.clone();

        let dc = dupe_checker.clone();
        if dc(&p, db, &prog)? {
            duplicate += 1;
            continue;
        }

        let tags = audiotags::Tag::new()
            .read_from_path(p.clone())
            .with_context(|| format!("Cannot read tags from {}", p.clone()))?;

        let mut track: model::Track = model::RawTrack { tags, path: p }.into();
        track.file_state = FileState::Copied;

        db.insert_track(&track)
            .await
            .with_context(|| format!("Cannot write track data to database"))?;

        prog.set_message(format!(
            "{}\nFound track: {} - {}, from {}",
            base_msg.clone(),
            track.title,
            track.artist,
            track.album
        ));

        new_tracks += 1;
    }

    prog.finish();
    mp.remove(&prog);

    db.insert_directory(path).await?;

    Ok((new_tracks, duplicate))
}
