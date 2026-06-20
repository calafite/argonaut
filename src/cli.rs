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
        #[arg(short = 'I', long = "include")]
        include_dirs: Vec<String>,
    },
    /// Compile (debug mode, sanitizers) and run
    Debug {
        file: PathBuf,
        #[arg(long, conflicts_with = "no_input")]
        input: bool,
        #[arg(long)]
        no_input: bool,
        #[arg(short = 'I', long = "include")]
        include_dirs: Vec<String>,
    },
    /// Rebuild and run on every save
    Watch {
        file: PathBuf,
        #[arg(long, conflicts_with = "no_input")]
        input: bool,
        #[arg(long)]
        no_input: bool,
        #[arg(short = 'I', long = "include")]
        include_dirs: Vec<String>,
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
    /// Bundle a solution file into a single monolithic file
    Bundle {
        file: PathBuf,
        #[arg(short, long)]
        out: Option<PathBuf>,
        #[arg(short = 'I', long = "include")]
        include_dirs: Vec<String>,
    },
}
