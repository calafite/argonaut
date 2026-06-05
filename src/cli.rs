use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "argonaut")]
#[command(version = "1.0.0")]
#[command(about = "Competitive Programming Toolkit", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Compile (release) and run
    Tcomp {
        /// The C++ source file
        file: PathBuf,
        /// Force read from input.txt (bypasses prompt)
        #[arg(long, conflicts_with = "no_input")]
        input: bool,
        /// Force interactive stdin (bypasses prompt)
        #[arg(long)]
        no_input: bool,
    },
    /// Compile (debug + sanitizers) and run
    Dbg {
        /// The C++ source file
        file: PathBuf,
        /// Force read from input.txt (bypasses prompt)
        #[arg(long, conflicts_with = "no_input")]
        input: bool,
        /// Force interactive stdin (bypasses prompt)
        #[arg(long)]
        no_input: bool,
    },
    /// Rebuild and run on every save
    Watchcp {
        /// The C++ source file
        file: PathBuf,
    },
    /// Scaffold a new solution file
    Mkcp {
        /// Directory to create
        dir: PathBuf,
        /// Filename (defaults to main)
        #[arg(default_value = "main")]
        name: String,
    },
    /// Execute a compiled binary directly
    Run {
        /// The compiled binary
        binary: PathBuf,
        /// Force read from input.txt (bypasses prompt)
        #[arg(long, conflicts_with = "no_input")]
        input: bool,
        /// Force interactive stdin (bypasses prompt)
        #[arg(long)]
        no_input: bool,
    },
}
