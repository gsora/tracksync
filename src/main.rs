use clap::Parser;
mod cli;
mod cmd;
mod db;
mod filter;
mod fs;
mod model;

#[async_std::main]
async fn main() -> anyhow::Result<()> {
    log_setup();

    let c = cli::Cli::parse();

    match c.command {
        cli::Commands::Sync(sync_args) => Ok(cmd::sync::run(sync_args).await?),
        cli::Commands::Add(add_args) => Ok(cmd::add::run(add_args, false).await?),
        cli::Commands::Dupes(dupes_args) => Ok(cmd::dupes::run(dupes_args).await?),
        cli::Commands::Clean(clean_args) => Ok(cmd::clean::run(clean_args).await?),
        cli::Commands::Update(update_args) => Ok(cmd::add::run(update_args, true).await?),
        cli::Commands::Filter(filter_args) => Ok(cmd::filter::run(filter_args).await?),
    }
}

fn log_setup() {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info")
    }

    env_logger::init();
}
