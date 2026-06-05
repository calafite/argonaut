use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "argo")]
#[command(version = "1.0.0")]
#[command(about = "Competitive Programming Toolkit", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Compile (release) and run
    Comp {
        file: PathBuf,
        #[arg(long, conflicts_with = "no_input")]
        input: bool,
        #[arg(long)]
        no_input: bool,
    },
    /// Compile (debug mode, sanitizers) and run
    Debug {
        file: PathBuf,
        #[arg(long, conflicts_with = "no_input")]
        input: bool,
        #[arg(long)]
        no_input: bool,
    },
    /// Rebuild and run on every save
    Watch {
        file: PathBuf,
        #[arg(long, conflicts_with = "no_input")]
        input: bool,
        #[arg(long)]
        no_input: bool,
    },
    /// Scaffold a new solution file
    Mkcp {
        dir: PathBuf,
        #[arg(default_value = "main")]
        name: String,
    },
    /// Execute a compiled binary directly
    Run {
        binary: PathBuf,
        #[arg(long, conflicts_with = "no_input")]
        input: bool,
        #[arg(long)]
        no_input: bool,
    },
}
