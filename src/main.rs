mod cli;
mod ui;

fn main() {
    println!("Hello, world!");
    mod cli;
    mod compiler;
    mod runner;
    mod ui;

    use anyhow::Result;
    use clap::Parser;
    use cli::{Cli, Commands};
    use compiler::Compiler;
    use runner::Runner;
    use ui::Ui;

    fn main() -> Result<()> {
        let cli = Cli::parse();

        match cli.command {
            Commands::Tcomp {
                file,
                input,
                no_input,
            } => {
                Ui::section("Release Build");
                Ui::meta("source", file.display());

                let binary = Compiler::build(&file, false)?;
                Ui::section("Running Program");
                Runner::run(&binary, input, no_input)?;
            }
            Commands::Dbg {
                file,
                input,
                no_input,
            } => {
                Ui::section("Debug Build");
                Ui::meta("source", file.display());

                let binary = Compiler::build(&file, true)?;
                Ui::section("Running Program");
                Runner::run(&binary, input, no_input)?;
            }
            Commands::Run {
                binary,
                input,
                no_input,
            } => {
                Ui::section("Running Program");
                Runner::run(&binary, input, no_input)?;
            }
            Commands::Watchcp { file } => {
                Ui::section("Watch Mode");
                Ui::meta("source", file.display());
                // TODO: Phase 3 (Watcher)
            }
            Commands::Mkcp { dir, name } => {
                Ui::section("Project Scaffold");
                // TODO: Phase 3 (Scaffolding)
            }
        }

        Ok(())
    }
}
