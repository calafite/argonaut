use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::bundler::Bundler;
use crate::config::settings::Config;
use crate::core::{compiler::Compiler, runner::Runner, scaffold::Scaffold, watcher::Watcher};
use crate::utils::{paths::get_include_dirs, ui::Ui};

#[derive(Parser)]
#[command(name = "argo")]
#[command(version = "1.0.0")]
#[command(about = "Competitive Programming Toolkit", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

impl Cli {
    pub fn execute(self) -> Result<()> {
        let config = Config::load()?;

        match self.command {
            Commands::Comp {
                file,
                input,
                no_input,
                include_dirs,
            } => {
                let use_file = Runner::resolve_input(input, no_input)?;
                let dirs = get_include_dirs(&include_dirs, &config, &file);

                Ui::section("Release Build");
                Ui::meta("source", file.display());

                let binary = Compiler::build(&file, false, &dirs)?;
                Ui::section("Running Program");
                Runner::run(&binary, use_file)?;
            }
            Commands::Debug {
                file,
                input,
                no_input,
                include_dirs,
            } => {
                let use_file = Runner::resolve_input(input, no_input)?;
                let dirs = get_include_dirs(&include_dirs, &config, &file);

                Ui::section("Debug Build");
                Ui::meta("source", file.display());

                let binary = Compiler::build(&file, true, &dirs)?;
                Ui::section("Running Program");
                Runner::run(&binary, use_file)?;
            }
            Commands::Run {
                binary,
                input,
                no_input,
            } => {
                let use_file = Runner::resolve_input(input, no_input)?;
                Ui::section("Running Program");
                Runner::run(&binary, use_file)?;
            }
            Commands::Watch {
                file,
                input,
                no_input,
                include_dirs,
            } => {
                let use_file = Runner::resolve_input(input, no_input)?;
                let dirs = get_include_dirs(&include_dirs, &config, &file);

                Watcher::watch(&file, use_file, &dirs)?;
            }
            Commands::Mkcp { dir, name } => {
                Ui::section("Project Scaffold");
                Scaffold::create(&dir, &name, &config)?;
            }
            Commands::Bundle {
                file,
                out,
                include_dirs,
            } => {
                let dirs = get_include_dirs(&include_dirs, &config, &file);

                Ui::section("Bundler");
                Ui::meta("source", file.display());

                let bundler = Bundler::new(dirs);
                let bundled = bundler.bundle(&file)?;

                let out_path = out.unwrap_or_else(|| {
                    let stem = file.file_stem().unwrap_or_default().to_string_lossy();
                    file.with_file_name(format!("{}_bundled.cpp", stem))
                });

                std::fs::write(&out_path, bundled)?;
                Ui::ok(format!("Bundled to {}", out_path.display()));
            }
        }

        Ok(())
    }
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
