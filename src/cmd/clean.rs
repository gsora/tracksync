use super::error;
use crate::{db, model};
use anyhow::{Context, Result};
use clap::Args as ClapArgs;

#[derive(ClapArgs, Debug)]
pub struct Args {
    #[arg(long)]
    pub destination: Option<String>,
}

impl Args {
    pub fn validate(&self) -> Result<(), error::Error> {
        if let None = self.destination {
            return Err(error::Error::ValidationError(
                "missing destination".to_owned(),
            ));
        };

        Ok(())
    }
}

pub async fn run(args: Args) -> Result<()> {
    args.validate()?;

    let dest_dir = args.destination.unwrap();

    let dest_db = db::Instance::new(&dest_dir, true).await?;

    for track in dest_db.tracks_by_state(model::FileState::Copying).await? {
        log::info!(
            "Deleting non-cleanly copied track: {} - {}, from {}",
            track.title,
            track.artist,
            track.album,
        );

        let storage = track.storage_path(&dest_dir);
        dest_db.delete(track.id).await?;
        std::fs::remove_file(storage.clone())
            .with_context(|| format!("Cannot delete file {}", storage.clone()))?;
    }

    Ok(())
}
