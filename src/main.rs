mod bundler;
mod cli;
mod compiler;
mod config;
mod runner;
mod scaffold;
mod ui;
mod watcher;

use anyhow::Result;
use bundler::Bundler;
use clap::Parser;
use cli::{Cli, Commands};
use compiler::Compiler;
use config::Config;
use runner::Runner;
use scaffold::Scaffold;
use ui::Ui;
use watcher::Watcher;

fn main() -> Result<()> {
    let cli = Cli::parse();

    let config = Config::load()?;

    match cli.command {
        Commands::Comp {
            file,
            input,
            no_input,
        } => {
            let use_file = Runner::resolve_input(input, no_input)?;
            let include_dirs: Vec<_> = config
                .build
                .include_dirs
                .iter()
                .map(|p| Config::expand_path(p))
                .collect();
            Ui::section("Release Build");
            Ui::meta("source", file.display());

            let binary = Compiler::build(&file, false, &include_dirs)?;
            Ui::section("Running Program");
            Runner::run(&binary, use_file)?;
        }
        Commands::Debug {
            file,
            input,
            no_input,
        } => {
            let use_file = Runner::resolve_input(input, no_input)?;
            let include_dirs: Vec<_> = config
                .build
                .include_dirs
                .iter()
                .map(|p| Config::expand_path(p))
                .collect();
            Ui::section("Debug Build");
            Ui::meta("source", file.display());

            let binary = Compiler::build(&file, true, &include_dirs)?;
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
        } => {
            let use_file = Runner::resolve_input(input, no_input)?;
            let include_dirs: Vec<_> = config
                .build
                .include_dirs
                .iter()
                .map(|p| Config::expand_path(p))
                .collect();
            Watcher::watch(&file, use_file, &include_dirs)?;
        }
        Commands::Mkcp { dir, name } => {
            Ui::section("Project Scaffold");
            Scaffold::create(&dir, &name, &config)?;
        }
        Commands::Bundle { file, out } => {
            let include_dirs: Vec<_> = config
                .build
                .include_dirs
                .iter()
                .map(|p| Config::expand_path(p))
                .collect();

            Ui::section("Bundler");
            Ui::meta("source", file.display());
            let mut bundler = Bundler::new(include_dirs);
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
