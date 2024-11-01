use crate::{cmd::error, db, filter};
use anyhow::{anyhow, Context, Result};
use clap::Args as ClapArgs;

const DEFAULT_FILTER: &'static str = include_str!("default_filter.rhai");

#[derive(ClapArgs)]
pub struct Args {
    /// Path where to store tunesdirector's database, as well as music files.
    #[arg(long)]
    pub destination: Option<String>,

    /// Read existing filters off the database.
    #[arg(long)]
    pub read: bool,

    /// Read filtering code from the specified file path, and store it in the database overwriting previously
    /// stored filters.
    /// This option will not open $EDITOR.
    #[arg(long)]
    pub file: Option<String>,
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

    let destination = args.destination.unwrap();

    let dest_db = db::Instance::new(&destination, true).await?;

    let existing_filter = dest_db
        .filter()
        .await
        .with_context(|| "Cannot fetch stored filter")?
        .unwrap_or(DEFAULT_FILTER.to_string());

    if args.read {
        println!("{}", existing_filter);
        return Ok(());
    }

    let res = match args.file {
        None => edit::edit(existing_filter)
            .with_context(|| "Cannot open $EDITOR to edit filtering code.")?,
        Some(path) => {
            let raw = std::fs::read(path).with_context(|| "Cannot read filter code path")?;

            String::from_utf8(raw).unwrap()
        }
    };

    filter::check(vec![res.clone()]).with_context(|| "Filtering script check failed")?;

    Ok(dest_db
        .set_filter(res)
        .await
        .with_context(|| "Cannot store filter")?)
}
