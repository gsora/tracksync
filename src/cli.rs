use clap::{command, Parser, Subcommand};

use crate::cmd;

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Syncs source and destination, based on the database constructed with 'add`.
    Sync(cmd::sync::Args),

    /// Adds a directory's content to the database.
    Add(cmd::add::Args),

    /// Finds albums that are stored in more than one audio format.
    Dupes(cmd::dupes::Args),

    /// Updates the local database with new tracks from previously added directories.
    Update(cmd::add::Args),

    /// Cleans destination of uncleanly-copied files.
    Clean(cmd::clean::Args),

    /// Filter tracks to copy over to a destination.
    Filter(cmd::filter::Args),
}
