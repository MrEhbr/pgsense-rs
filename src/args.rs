use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::commands;

#[derive(Parser)]
#[command(version, about)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// List and test detection rules
    Rules(commands::rules::Args),

    /// Start scanning PostgreSQL replication stream for sensitive data
    Scan(commands::scan::Args),
}

pub async fn route(args: Args) -> Result<()> {
    match args.command {
        Command::Rules(cmd_args) => commands::rules::run(cmd_args),
        Command::Scan(cmd_args) => commands::scan::run(cmd_args).await,
    }
}
