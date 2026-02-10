#![forbid(unsafe_code)]

use clap::Parser;
use pgsense_rs::args::{self, Args};

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if let Err(error) = args::route(args).await {
        eprintln!("Error: {:?}", error);
        std::process::exit(1);
    }
}
