mod cli;
mod compiler;
mod runner;
mod scaffold;
mod ui;
mod watcher;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};
use compiler::Compiler;
use runner::Runner;
use scaffold::Scaffold;
use ui::Ui;
use watcher::Watcher;

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Tcomp {
            file,
            input,
            no_input,
        } => {
            let use_file = Runner::resolve_input(input, no_input)?;
            Ui::section("Release Build");
            Ui::meta("source", file.display());

            let binary = Compiler::build(&file, false)?;
            Ui::section("Running Program");
            Runner::run(&binary, use_file)?;
        }
        Commands::Dbg {
            file,
            input,
            no_input,
        } => {
            let use_file = Runner::resolve_input(input, no_input)?;
            Ui::section("Debug Build");
            Ui::meta("source", file.display());

            let binary = Compiler::build(&file, true)?;
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
        Commands::Watchcp {
            file,
            input,
            no_input,
        } => {
            let use_file = Runner::resolve_input(input, no_input)?;
            Watcher::watch(&file, use_file)?;
        }
        Commands::Mkcp { dir, name } => {
            Ui::section("Project Scaffold");
            Scaffold::create(&dir, &name)?;
        }
    }

    Ok(())
}
