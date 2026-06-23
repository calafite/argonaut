use anyhow::Result;
use clap::builder::styling::{AnsiColor, Effects, Styles};
use clap::{Parser, Subcommand, ValueHint};
use std::path::PathBuf;

use crate::bundler::Bundler;
use crate::config::settings::Config;
use crate::core::{compiler::Compiler, formatter::Formatter, runner::Runner, scaffold::Scaffold};
use crate::utils::{paths::get_include_dirs, ui::Ui};

fn cli_styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Yellow.on_default() | Effects::BOLD)
        .usage(AnsiColor::Green.on_default() | Effects::BOLD)
        .literal(AnsiColor::Cyan.on_default() | Effects::BOLD)
        .placeholder(AnsiColor::Green.on_default())
        .error(AnsiColor::Red.on_default() | Effects::BOLD)
        .valid(AnsiColor::Cyan.on_default() | Effects::BOLD)
        .invalid(AnsiColor::Yellow.on_default() | Effects::BOLD)
}

#[derive(Parser)]
#[command(name = "argo")]
#[command(version = "1.0.0")]
#[command(about = "Competitive Programming Toolkit", long_about = None)]
#[command(styles = cli_styles())]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

impl Cli {
    pub fn execute(self) -> Result<()> {
        let config = Config::load()?;

        match self.command {
            Commands::Build { file, include_dirs } => {
                let dirs = get_include_dirs(&include_dirs, &config, &file);

                Ui::section("Release Build");
                Ui::meta("source", file.display());
                Compiler::build(&file, false, &dirs)?;
            }
            Commands::Debug { file, include_dirs } => {
                let dirs = get_include_dirs(&include_dirs, &config, &file);

                Ui::section("Debug Build");
                Ui::meta("source", file.display());
                Compiler::build(&file, true, &dirs)?;
            }
            Commands::Test {
                target,
                input,
                no_input,
            } => {
                let (binary, display_name) = Compiler::resolve_test_target(target.as_deref())?;

                let use_file = Runner::resolve_input(input, no_input)?;
                Ui::section("Running Tests");
                Ui::meta("target", display_name);
                Runner::run(&binary, use_file)?;
            }
            Commands::New { dir, name } => {
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
            Commands::Format { file } => {
                Ui::section("Code Formatter");
                Formatter::format(&file)?;
            }
        }

        Ok(())
    }
}

#[derive(Subcommand)]
pub enum Commands {
    /// Compile solution in release mode (-O2)
    Build {
        file: PathBuf,
        #[arg(short = 'I', long = "include")]
        include_dirs: Vec<String>,
    },
    /// Compile solution with debug symbols & sanitizers
    Debug {
        file: PathBuf,
        #[arg(short = 'I', long = "include")]
        include_dirs: Vec<String>,
    },
    /// Execute a compiled solution against inputs
    Test {
        #[arg(value_hint = ValueHint::FilePath)]
        target: Option<String>,
        #[arg(long, conflicts_with = "no_input")]
        input: bool,
        #[arg(long)]
        no_input: bool,
    },
    /// Scaffold a new solution file
    New {
        dir: PathBuf,
        #[arg(default_value = "main")]
        name: String,
    },
    /// Bundle a solution file into a single monolithic file
    Bundle {
        file: PathBuf,
        #[arg(short, long)]
        out: Option<PathBuf>,
        #[arg(short = 'I', long = "include")]
        include_dirs: Vec<String>,
    },
    /// Format a C++ solution using a CP-optimized profile
    Format {
        #[arg(value_hint = ValueHint::FilePath)]
        file: PathBuf,
    },
}
