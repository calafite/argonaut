use anyhow::Result;
use clap::builder::styling::{AnsiColor, Effects, Styles};
use clap::{Parser, Subcommand, ValueHint};
use std::path::{Path, PathBuf};

use crate::bundler::Bundler;
use crate::config::settings::Configuration;
use crate::core::{compiler::Compiler, formatter::Formatter, runner::Runner, scaffold::Scaffold};
use crate::utils::ui::Ui;

#[derive(Parser)]
#[command(name = "argo")]
#[command(version = "1.0.0")]
#[command(about = "Competitive Programming Toolkit", long_about = None)]
#[command(styles = styles())]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

impl Cli {
    pub fn execute(self) -> Result<()> {
        let config = Configuration::load()?;

        match self.command {
            Commands::Build {
                file,
                include_dirs,
                std,
                mode,
            } => Self::handle_build(file, include_dirs, std, mode, &config),
            Commands::Debug {
                file,
                include_dirs,
                std,
            } => Self::handle_debug(file, include_dirs, std, &config),
            Commands::Test {
                target,
                input,
                no_input,
            } => Self::handle_test(target, input, no_input),
            Commands::New { name } => Self::handle_new(name, &config),
            Commands::Bundle {
                file,
                out,
                include_dirs,
                minify,
            } => Self::handle_bundle(file, out, include_dirs, minify, &config),
            Commands::Format { file } => Self::handle_format(file),
            Commands::Peek {
                file,
                out,
                debug,
                reduced,
                include_dirs,
                std,
                mode,
            } => Self::handle_peek(file, out, debug, reduced, include_dirs, std, mode, &config),
        }
    }

    fn handle_build(
        file: PathBuf,
        include_dirs: Vec<String>,
        std: Option<u32>,
        mode: Option<String>,
        config: &Configuration,
    ) -> Result<()> {
        let std_version = std.unwrap_or(config.build.std);
        let directories = get_includes(&include_dirs, config, &file);
        Self::print_metadata("Release Build", &file, &config.build.compiler, std_version);
        let mode = mode.unwrap_or_default().to_lowercase();
        let mode: &'static str = Box::leak(mode.into_boxed_str());
        let compiler_cmd: &'static str = Box::leak(config.build.compiler.clone().into_boxed_str());

        Compiler::build(
            &file,
            false,
            &directories,
            compiler_cmd,
            std_version,
            config.build.log_file,
            mode,
        )
        .map(|_| ())
    }

    fn handle_debug(
        file: PathBuf,
        include_dirs: Vec<String>,
        std: Option<u32>,
        config: &Configuration,
    ) -> Result<()> {
        let std_version = std.unwrap_or(config.build.std);
        let directories = get_includes(&include_dirs, config, &file);
        static EMPTY: &str = "";
        let compiler_cmd: &'static str = Box::leak(config.build.compiler.clone().into_boxed_str());

        Self::print_metadata("Debug Build", &file, &config.build.compiler, std_version);

        Compiler::build(
            &file,
            true,
            &directories,
            compiler_cmd,
            std_version,
            config.build.log_file,
            EMPTY,
        )
        .map(|_| ())
    }

    fn handle_test(target: Option<String>, input: bool, no_input: bool) -> Result<()> {
        let (binary, display) = Compiler::resolve_test_target(target.as_deref())?;
        let use_file = Runner::resolve_input(&binary, input, no_input)?;

        Ui::section("Running Tests");
        Ui::meta("target", display);
        Runner::run(&binary, use_file)
    }

    fn handle_new(name: String, config: &Configuration) -> Result<()> {
        Ui::section("Project Scaffold");
        Scaffold::create(&name, config)
    }

    fn handle_bundle(
        file: PathBuf,
        out: Option<PathBuf>,
        include_dirs: Vec<String>,
        minify: bool,
        config: &Configuration,
    ) -> Result<()> {
        let directories = get_includes(&include_dirs, config, &file);

        Ui::section("Bundler");
        Ui::meta("source", file.display());

        let bundler = Bundler::new(directories);
        let mut bundled = bundler.bundle(&file)?;

        if minify {
            Ui::meta("minify", "enabled");
            let original_len = bundled.len();
            bundled = crate::bundler::minify::Minifier::minify(&bundled);
            let new_len = bundled.len();
            Ui::info(format!(
                "Compressed from {} bytes to {} bytes ({:.1}% reduction)",
                original_len,
                new_len,
                100.0 - (new_len as f64 / original_len as f64) * 100.0
            ));
        }

        let out_path = out.unwrap_or_else(|| {
            let stem = file.file_stem().unwrap_or_default().to_string_lossy();
            file.with_file_name(format!("{}_bundled.cpp", stem))
        });

        std::fs::write(&out_path, bundled)?;
        Ui::ok(format!("bundled to {}", out_path.display()));
        Ok(())
    }

    fn handle_format(file: PathBuf) -> Result<()> {
        Ui::section("Code Formatter");
        Formatter::format(&file)
    }

    fn handle_peek(
        file: PathBuf,
        out: Option<PathBuf>,
        debug: bool,
        reduced: bool,
        include_dirs: Vec<String>,
        std: Option<u32>,
        mode: Option<String>,
        config: &Configuration,
    ) -> Result<()> {
        let std_version = std.unwrap_or(config.build.std);
        let directories = get_includes(&include_dirs, config, &file);

        Ui::section("Assembly Peek");
        Ui::meta("source", file.display());

        let compiler = if reduced {
            Compiler::cross_compiler()
        } else {
            config.build.compiler.clone()
        };
        let compiler_cmd: &'static str = Box::leak(compiler.clone().into_boxed_str());

        Ui::meta("compiler", &compiler);
        Ui::meta("std", format!("C++{}", std_version));

        let mode = mode.unwrap_or_default().to_lowercase();
        let mode: &'static str = Box::leak(mode.into_boxed_str());

        Compiler::peek(
            &file,
            out.as_deref(),
            debug,
            &directories,
            compiler_cmd,
            std_version,
            &mode,
        )
        .map(|_| ())
    }

    fn print_metadata(section: &str, file: &Path, compiler: &str, std_version: u32) {
        Ui::section(section);
        Ui::meta("source", file.display());
        Ui::meta("compiler", compiler);
        Ui::meta("std", format!("C++{}", std_version));
    }
}

#[derive(Subcommand)]
pub enum Commands {
    /// Compile solution in release mode (-O2)
    Build {
        file: PathBuf,
        #[arg(short = 'I', long = "include")]
        include_dirs: Vec<String>,
        #[arg(short = 's', long = "std")]
        std: Option<u32>,
        #[arg(short = 'm', long = "mode")]
        mode: Option<String>,
    },
    /// Compile solution with debug symbols & sanitizers
    Debug {
        file: PathBuf,
        #[arg(short = 'I', long = "include")]
        include_dirs: Vec<String>,
        #[arg(short = 's', long = "std")]
        std: Option<u32>,
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
    New { name: String },
    /// Bundle a solution file into a single monolithic file
    Bundle {
        file: PathBuf,
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// Directories to compile against.
        #[arg(short = 'I', long = "include")]
        include_dirs: Vec<String>,
        // Aggressively compact the code.
        #[arg(short = 'M', long)]
        minify: bool,
    },
    /// Format a C++ solution using a CP-optimized profile
    Format {
        #[arg(value_hint = ValueHint::FilePath)]
        file: PathBuf,
    },
    /// Output the intermediate assembly generated in the build
    Peek {
        #[arg(value_hint = ValueHint::FilePath)]
        file: PathBuf,
        #[arg(short, long)]
        out: Option<PathBuf>,
        #[arg(long)]
        debug: bool,
        #[arg(long)]
        reduced: bool,
        #[arg(short = 'I', long = "include")]
        include_dirs: Vec<String>,
        #[arg(short = 's', long = "std")]
        std: Option<u32>,
        #[arg(short = 'm', long = "mode")]
        mode: Option<String>,
    },
}

fn styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Yellow.on_default() | Effects::BOLD)
        .usage(AnsiColor::Green.on_default() | Effects::BOLD)
        .literal(AnsiColor::Cyan.on_default() | Effects::BOLD)
        .placeholder(AnsiColor::Green.on_default())
        .error(AnsiColor::Red.on_default() | Effects::BOLD)
        .valid(AnsiColor::Cyan.on_default() | Effects::BOLD)
        .invalid(AnsiColor::Yellow.on_default() | Effects::BOLD)
}

fn get_includes(include_dirs: &[String], config: &Configuration, file: &Path) -> Vec<PathBuf> {
    let mut directories: Vec<PathBuf> = include_dirs.iter().map(PathBuf::from).collect();
    for directory in &config.build.include_dirs {
        directories.push(PathBuf::from(directory));
    }
    if let Some(parent) = file.parent() {
        directories.push(parent.to_path_buf());
    }
    directories
}
