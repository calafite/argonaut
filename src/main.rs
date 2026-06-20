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
use std::path::{Path, PathBuf};
use ui::Ui;
use watcher::Watcher;

fn get_include_dirs(cli_includes: &[String], config: &Config, file: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<_> = config
        .build
        .include_dirs
        .iter()
        .map(|p| Config::expand_path(p))
        .collect();

    for inc in cli_includes {
        dirs.push(Config::expand_path(inc));
    }

    // Resolve absolute paths to guarantee parent detection works
    let abs_file = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());
    
    // Auto-detect a local `include` directory next to the target file
    if let Some(parent) = abs_file.parent() {
        let parent_include = parent.join("include");
        if parent_include.exists() {
            dirs.push(parent_include);
        }
        dirs.push(parent.to_path_buf());
    }
    
    // Auto-detect `include` directory from the terminal's Current Working Directory
    if let Ok(cwd) = std::env::current_dir() {
        let cwd_include = cwd.join("include");
        if cwd_include.exists() {
            dirs.push(cwd_include);
        }
        dirs.push(cwd);
    }
    
    // Deduplicate to prevent overlapping scans
    let mut resolved = Vec::new();
    for d in dirs {
        let canon = d.canonicalize().unwrap_or(d);
        if !resolved.contains(&canon) {
            resolved.push(canon);
        }
    }
    
    resolved
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = Config::load()?;

    match cli.command {
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
        Commands::Bundle { file, out, include_dirs } => {
            let dirs = get_include_dirs(&include_dirs, &config, &file);
                
            Ui::section("Bundler");
            Ui::meta("source", file.display());
            let mut bundler = Bundler::new(dirs);
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
