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

fn find_cross_compiler() -> String {
    let candidates = [
        "riscv64-buildroot-linux-gnu-g++",
        "riscv64-linux-gnu-g++",
        "riscv64-unknown-linux-gnu-g++",
    ];
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            for candidate in candidates {
                let candidate_path = dir.join(candidate);
                if candidate_path.exists() {
                    return candidate.to_string();
                }
            }
        }
    }
    "riscv64-linux-gnu-g++".to_string()
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
            Commands::Build {
                file,
                include_dirs,
                std,
                mode,
            } => {
                let std_version = std.unwrap_or(config.build.std);
                let dirs = get_include_dirs(&include_dirs, &config, &file);

                Ui::section("Release Build");
                Ui::meta("source", file.display());
                Ui::meta("compiler", &config.build.compiler);
                Ui::meta("std", format!("C++{}", std_version));

                let mode = match mode {
                    Some(m) => m.to_lowercase(),
                    None => String::new(),
                };

                Compiler::build(
                    &file,
                    false,
                    &dirs,
                    &config.build.compiler,
                    std_version,
                    config.build.log_file,
                    mode,
                )?;
            }
            Commands::Debug {
                file,
                include_dirs,
                std,
            } => {
                let std_version = std.unwrap_or(config.build.std);
                let dirs = get_include_dirs(&include_dirs, &config, &file);

                Ui::section("Debug Build");
                Ui::meta("source", file.display());
                Ui::meta("compiler", &config.build.compiler);
                Ui::meta("std", format!("C++{}", std_version));

                Compiler::build(
                    &file,
                    true,
                    &dirs,
                    &config.build.compiler,
                    std_version,
                    config.build.log_file,
                    "".to_string(),
                )?;
            }
            Commands::Test {
                target,
                input,
                no_input,
            } => {
                let (binary, display_name) = Compiler::resolve_test_target(target.as_deref())?;

                let use_file = Runner::resolve_input(&binary, input, no_input)?;
                Ui::section("Running Tests");
                Ui::meta("target", display_name);
                Runner::run(&binary, use_file)?;
            }
            Commands::New { name } => {
                Ui::section("Project Scaffold");
                Scaffold::create(&name, &config)?;
            }
            Commands::Bundle {
                file,
                out,
                include_dirs,
                minify,
            } => {
                let dirs = get_include_dirs(&include_dirs, &config, &file);

                Ui::section("Bundler");
                Ui::meta("source", file.display());

                let bundler = Bundler::new(dirs);
                let mut bundled = bundler.bundle(&file)?;

                if minify {
                    Ui::meta("minify", "enabled");
                    let original_len = bundled.len();

                    bundled = crate::bundler::minify::minify_bundle(&bundled);

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
            }
            Commands::Format { file } => {
                Ui::section("Code Formatter");
                Formatter::format(&file)?;
            }
            Commands::Peek {
                file,
                out,
                debug,
                reduced,
                include_dirs,
                std,
                mode,
            } => {
                let std_version = std.unwrap_or(config.build.std);
                let dirs = get_include_dirs(&include_dirs, &config, &file);

                Ui::section("Assembly Peek");
                Ui::meta("source", file.display());
                let compiler = if reduced {
                    find_cross_compiler()
                } else {
                    config.build.compiler.clone()
                };
                Ui::meta("compiler", &compiler);
                Ui::meta("std", format!("C++{}", std_version));

                let mode = match mode {
                    Some(ms) => ms.to_lowercase(),
                    None => String::new(),
                };

                Compiler::peek(
                    &file,
                    out.as_deref(),
                    debug,
                    &dirs,
                    &compiler,
                    std_version,
                    mode,
                )?;
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
