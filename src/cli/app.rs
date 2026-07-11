use anyhow::Result;
use clap::builder::styling::{AnsiColor, Effects, Styles};
use clap::{Args, Parser, Subcommand, ValueHint};
use std::path::PathBuf;

use crate::bundler::Bundler;
use crate::config::settings::Configuration;
use crate::core::compiler::Compiler;
use crate::core::formatter::Formatter;
use crate::core::profiler::Profiler;
use crate::core::scaffold::Scaffold;
use crate::core::tester::Tester;
use crate::parser::server::ProblemListener;

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
            Commands::Profile(args) => args.execute(&config),
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
    /// Attach performance profilers to evaluate execution bottlenecks
    Profile(ProfileArgs),
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
        Compiler::execute_build(
            &self.file,
            &self.include_dirs,
            self.std,
            self.mode.as_deref(),
            config,
        )
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
        Compiler::execute_debug(&self.file, &self.include_dirs, self.std, config)
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
        Tester::execute_test(self.target.as_deref(), self.input, self.no_input)
    }
}

#[derive(Args)]
pub struct NewArgs {
    pub name: String,
}

impl NewArgs {
    pub fn execute(self, config: &Configuration) -> Result<()> {
        Scaffold::execute_new(&self.name, config)
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
        Bundler::execute_bundle(
            &self.file,
            self.out.as_deref(),
            &self.include_dirs,
            self.minify,
            config,
        )
    }
}

#[derive(Args)]
pub struct FormatArgs {
    #[arg(value_hint = ValueHint::FilePath)]
    pub file: PathBuf,
}

impl FormatArgs {
    pub fn execute(self) -> Result<()> {
        Formatter::execute_format(&self.file)
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
        Compiler::execute_peek(
            &self.file,
            self.out.as_deref(),
            self.debug,
            self.reduced,
            &self.include_dirs,
            self.std,
            self.mode.as_deref(),
            config,
        )
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
        ProblemListener::execute_listen(self.port, self.short, config)
    }
}

#[derive(Args)]
pub struct ProfileArgs {
    #[arg(value_hint = ValueHint::FilePath)]
    pub target: Option<String>,
    #[arg(short, long)]
    pub input: Option<String>,
}

impl ProfileArgs {
    pub fn execute(self, config: &Configuration) -> Result<()> {
        Profiler::execute_profile(self.target.as_deref(), self.input.as_deref(), config)
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
