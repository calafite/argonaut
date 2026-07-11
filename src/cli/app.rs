use anyhow::Result;
use clap::builder::styling::{AnsiColor, Effects, Styles};
use clap::{Args, Parser, Subcommand, ValueHint};
use std::path::{Path, PathBuf};

use crate::bundler::Bundler;
use crate::config::settings::Configuration;
use crate::core::compiler::{BuildArguments, Compiler};
use crate::core::formatter::Formatter;
use crate::core::runner::Runner;
use crate::core::scaffold::Scaffold;
use crate::core::tester::Tester;
use crate::parser::server::ProblemListener;
use crate::utils::{paths::PathUtilities, ui::Ui};

#[derive(Parser)]
#[command(name = "argo")]
#[command(version = "0.5.0")]
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
            Commands::Build(args) => args.execute(&config),
            Commands::Debug(args) => args.execute(&config),
            Commands::Test(args) => args.execute(),
            Commands::New(args) => args.execute(&config),
            Commands::Bundle(args) => args.execute(&config),
            Commands::Format(args) => args.execute(),
            Commands::Peek(args) => args.execute(&config),
            Commands::Listen(args) => args.execute(&config),
        }
    }
}

#[derive(Subcommand)]
pub enum Commands {
    /// Compile solution in release mode (-O2)
    Build(BuildArgs),
    /// Compile solution with debug symbols & sanitizers
    Debug(DebugArgs),
    /// Execute a compiled solution against inputs
    Test(TestArgs),
    /// Scaffold a new solution file
    New(NewArgs),
    /// Bundle a solution file into a single monolithic file
    Bundle(BundleArgs),
    /// Format a C++ solution using a CP-optimized profile
    Format(FormatArgs),
    /// Output the intermediate assembly generated in the build
    Peek(PeekArgs),
    /// Listen for Competitive Companion problem payloads
    Listen(ListenArgs),
}

#[derive(Args)]
pub struct BuildArgs {
    pub file: PathBuf,
    #[arg(short = 'I', long = "include")]
    pub include_dirs: Vec<String>,
    #[arg(short = 's', long = "std")]
    pub std: Option<u32>,
    #[arg(short = 'm', long = "mode")]
    pub mode: Option<String>,
}

impl BuildArgs {
    pub fn execute(self, config: &Configuration) -> Result<()> {
        let std_version = self.std.unwrap_or(config.build.std);
        let directories = PathUtilities::get_include_dirs(&self.include_dirs, config, &self.file);

        print_metadata(
            "Release Build",
            &self.file,
            &config.build.compiler,
            std_version,
        );

        let mode = self.mode.unwrap_or_default().to_lowercase();

        let args = BuildArguments::new()
            .file(&self.file)
            .debug(false)
            .includes(&directories)
            .cmd(config.build.compiler.clone())
            .std(std_version)
            .log(config.build.log_file)
            .mode(mode);

        Compiler::build(args).map(|_| ())
    }
}

#[derive(Args)]
pub struct DebugArgs {
    pub file: PathBuf,
    #[arg(short = 'I', long = "include")]
    pub include_dirs: Vec<String>,
    #[arg(short = 's', long = "std")]
    pub std: Option<u32>,
}

impl DebugArgs {
    pub fn execute(self, config: &Configuration) -> Result<()> {
        let std_version = self.std.unwrap_or(config.build.std);
        let directories = PathUtilities::get_include_dirs(&self.include_dirs, config, &self.file);
        static EMPTY: &str = "";

        print_metadata(
            "Debug Build",
            &self.file,
            &config.build.compiler,
            std_version,
        );

        let args = BuildArguments::new()
            .file(&self.file)
            .debug(true)
            .includes(&directories)
            .cmd(config.build.compiler.clone())
            .std(std_version)
            .log(config.build.log_file)
            .mode(EMPTY);

        Compiler::build(args).map(|_| ())
    }
}

#[derive(Args)]
pub struct TestArgs {
    #[arg(value_hint = ValueHint::FilePath)]
    pub target: Option<String>,
    #[arg(long, conflicts_with = "no_input")]
    pub input: bool,
    #[arg(long)]
    pub no_input: bool,
}

impl TestArgs {
    pub fn execute(self) -> Result<()> {
        let (binary, display) = Compiler::resolve_test_target(self.target.as_deref())?;
        Ui::section("Running Tests");
        Ui::meta("target", display);
        let tests_run = Tester::run_suite(&binary)?;
        if tests_run > 0 {
            return Ok(());
        }
        let use_file = Runner::resolve_input(&binary, self.input, self.no_input)?;
        Runner::run(&binary, use_file)
    }
}

#[derive(Args)]
pub struct NewArgs {
    pub name: String,
}

impl NewArgs {
    pub fn execute(self, config: &Configuration) -> Result<()> {
        Ui::section("Project Scaffold");
        Scaffold::create(&self.name, config)
    }
}

#[derive(Args)]
pub struct BundleArgs {
    pub file: PathBuf,
    #[arg(short, long)]
    pub out: Option<PathBuf>,
    /// Directories to compile against.
    #[arg(short = 'I', long = "include")]
    pub include_dirs: Vec<String>,
    // Aggressively compact the code.
    #[arg(short = 'M', long)]
    pub minify: bool,
}

impl BundleArgs {
    pub fn execute(self, config: &Configuration) -> Result<()> {
        let directories = PathUtilities::get_include_dirs(&self.include_dirs, config, &self.file);

        Ui::section("Bundler");
        Ui::meta("source", self.file.display());

        let bundler = Bundler::new(directories);
        let mut bundled = bundler.bundle(&self.file)?;

        if self.minify {
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

        let out_path = self.out.unwrap_or_else(|| {
            let stem = self.file.file_stem().unwrap_or_default().to_string_lossy();
            self.file.with_file_name(format!("{}_bundled.cpp", stem))
        });

        std::fs::write(&out_path, bundled)?;
        Ui::ok(format!("bundled to {}", out_path.display()));
        Ok(())
    }
}

#[derive(Args)]
pub struct FormatArgs {
    #[arg(value_hint = ValueHint::FilePath)]
    pub file: PathBuf,
}

impl FormatArgs {
    pub fn execute(self) -> Result<()> {
        Ui::section("Code Formatter");
        Formatter::format(&self.file)
    }
}

#[derive(Args)]
pub struct PeekArgs {
    #[arg(value_hint = ValueHint::FilePath)]
    pub file: PathBuf,
    #[arg(short, long)]
    pub out: Option<PathBuf>,
    #[arg(long)]
    pub debug: bool,
    #[arg(long)]
    pub reduced: bool,
    #[arg(short = 'I', long = "include")]
    pub include_dirs: Vec<String>,
    #[arg(short = 's', long = "std")]
    pub std: Option<u32>,
    #[arg(short = 'm', long = "mode")]
    pub mode: Option<String>,
}

impl PeekArgs {
    pub fn execute(self, config: &Configuration) -> Result<()> {
        if Compiler::is_assembly(&self.file) {
            anyhow::bail!("Cannot peek assembly output of a pure assembly file.");
        }

        let std_version = self.std.unwrap_or(config.build.std);
        let directories = PathUtilities::get_include_dirs(&self.include_dirs, config, &self.file);

        Ui::section("Assembly Peek");
        Ui::meta("source", self.file.display());

        let compiler = if self.reduced {
            Compiler::cross_compiler(&config.build.compiler)
        } else {
            config.build.compiler.clone()
        };

        Ui::meta("compiler", &compiler);
        Ui::meta("std", format!("C++{}", std_version));

        let mode = self.mode.unwrap_or_default().to_lowercase();

        let args = BuildArguments::new()
            .file(&self.file)
            .debug(self.debug)
            .includes(&directories)
            .cmd(compiler.clone())
            .std(std_version)
            .mode(mode);

        Compiler::peek(args, self.out.as_deref()).map(|_| ())
    }
}

#[derive(Args)]
pub struct ListenArgs {
    /// Port to listen on for Competitive Companion payloads
    #[arg(short, long, default_value_t = 10045)]
    pub port: u16,
    /// Whether to use short codes as problem names when scaffolding.
    #[arg(short, long)]
    pub short: bool,
}

impl ListenArgs {
    pub fn execute(self, config: &Configuration) -> Result<()> {
        Ui::section("Problem Parser Server");
        let use_short = self.short || config.scaffold.short_name;
        ProblemListener::start(self.port, use_short, config)
    }
}

fn print_metadata(section: &str, file: &Path, compiler: &str, std_version: u32) {
    Ui::section(section);
    Ui::meta("source", file.display());
    Ui::meta("compiler", compiler);

    if Compiler::is_assembly(file) {
        let arch = Compiler::target_architecture(compiler)
            .map(|a| Compiler::format_arch(&a))
            .unwrap_or_else(|_| "Unknown".to_string());
        Ui::meta("type", format!("{} ASM", arch));
    } else {
        Ui::meta("std", format!("C++{}", std_version));
    }
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
