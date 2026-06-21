use anyhow::Result;
use clap::Parser;

use argonaut::cli::app::Cli;

fn main() -> Result<()> {
    Cli::parse().execute()
}
