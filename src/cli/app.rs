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
#[command(about = "High-performance Competitive Programming Toolkit in Rust")]
#[command(
    long_about = "Argonaut (argo) is an integrated CLI toolkit for competitive programmers.\n\
It automates project scaffolding, sandboxed testing, AST-level header bundling,\n\
automated crash stack traces, assembly inspection, and microarchitectural profiling."
)]
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
    /// Compile a solution in release mode (-O2/O3/Ofast)
    ///
    /// Compiles a C++ or Assembly solution into an optimized binary inside the local
    /// `.argo/` cache directory. Enables standard warning diagnostics (-Wall -Wextra -Wshadow)
    /// and defines the -DLOCAL preprocessor flag.
    ///
    /// EXAMPLES:
    ///   argo build solution.cpp
    ///   argo build solution.cpp -m ofast
    ///   argo build solution.cpp -s 23 -I ../library/include
    Build(BuildArgs),

    /// Compile with debug symbols & runtime sanitizers
    ///
    /// Compiles with `-g -O1` alongside AddressSanitizer and UndefinedBehaviorSanitizer
    /// (`-fsanitize=address,undefined`). Essential for diagnosing Segfaults, out-of-bounds
    /// array accesses, integer overflows, and misaligned pointers.
    ///
    /// EXAMPLES:
    ///   argo debug solution.cpp
    ///   argo debug solution.cpp -s 23
    Debug(DebugArgs),

    /// Execute solution against sample and custom test cases
    ///
    /// Executes the target binary against test fixtures in `.argo/tests/<name>/` (`in_*.txt` & `out_*.txt`).
    /// If no target is specified, Argonaut automatically selects the most recently compiled binary.
    ///
    /// Test execution is sandboxed using Linux Seccomp BPF filters and enforces the memory limit
    /// parsed directly from the solution's header comment. If the process crashes, Argonaut uses
    /// embedded GDB backtracing to identify the exact line of code that triggered the fault.
    ///
    /// EXAMPLES:
    ///   argo test                       # Auto-selects newest compiled binary
    ///   argo test A                     # Runs tests for problem 'A'
    ///   argo test solution.cpp --input  # Forces input from input.txt without prompting
    ///   argo test A --no-input          # Runs interactively from standard input
    Test(TestArgs),

    /// Scaffold a new solution workspace and directory
    ///
    /// Creates a dedicated problem directory, scaffolds `<name>.cpp` using the template
    /// path configured in `Config.toml` (or generates a boilerplate file), and prepares
    /// local caching.
    ///
    /// EXAMPLES:
    ///   argo new A
    ///   argo new 1900B
    ///   argo new shortest_paths
    New(NewArgs),

    /// Bundle multi-file solutions into a single monolithic file
    ///
    /// Uses Tree-sitter AST parsing to traverse `#include` directives, shakes out unused
    /// symbols/templates/classes (Dead Code Elimination), and emits a single submission-ready
    /// file. Also supports variable and preprocessor macro minification for byte-limited contests.
    ///
    /// EXAMPLES:
    ///   argo bundle solution.cpp
    ///   argo bundle solution.cpp -o submit.cpp
    ///   argo bundle solution.cpp -M -I ~/cplib/include
    Bundle(BundleArgs),

    /// Format C++ source files using a CP-optimized profile
    ///
    /// Formats the file using `clang-format`. Searches hierarchically for a local `.clang-format`
    /// configuration, falling back to Argonaut's built-in competitive-programming profile
    /// (short blocks on single line, bit/stdc++ grouping, lambda compactness).
    ///
    /// EXAMPLES:
    ///   argo format solution.cpp
    Format(FormatArgs),

    /// Inspect compiled assembly instructions (-S)
    ///
    /// Outputs the assembly emitted by the compiler. Supports cross-compiling to RISC-V 64
    /// (`--reduced`) to produce compact, readable instructions ideal for verifying SIMD
    /// loop auto-vectorization and unrolling without x86 ABI bloat.
    ///
    /// EXAMPLES:
    ///   argo peek solution.cpp
    ///   argo peek solution.cpp --reduced -m o3
    ///   argo peek solution.cpp -o custom.s
    Peek(PeekArgs),

    /// Listen for Competitive Companion browser payloads
    ///
    /// Launches a local HTTP daemon that listens for problem payloads sent by the Competitive
    /// Companion browser extension. When a problem is captured, Argonaut automatically scaffolds
    /// the problem directory, parses time and memory limits, and creates all sample test fixtures.
    ///
    /// EXAMPLES:
    ///   argo listen
    ///   argo listen --port 10045 --short
    Listen(ListenArgs),

    /// Run hardware performance counters and microarchitectural profilers
    ///
    /// Profiles your compiled binary across multiple complementary engines:
    ///   - Linux PMU (perf): Measures IPC, branch mispredictions, and L1 cache miss rates.
    ///   - Perf Sampling: Identifies exact hotspot functions and machine instruction addresses.
    ///   - Valgrind Cachegrind: Simulates cache reference locality.
    ///   - llvm-mca: Static pipeline throughput and hardware bottleneck analysis.
    ///   - Vectorization Report: Diagnoses missed SIMD optimization passes.
    ///
    /// EXAMPLES:
    ///   argo profile                           # Profiles the newest binary with default input
    ///   argo profile A -i custom_large_in.txt  # Feeds a custom benchmark input
    Profile(ProfileArgs),
}

#[derive(Args)]
pub struct BuildArgs {
    /// Source file to compile (.cpp, .s, or .asm)
    #[arg(value_hint = ValueHint::FilePath)]
    pub file: PathBuf,

    /// Additional include directories passed via -I (repeatable)
    #[arg(short = 'I', long = "include")]
    pub include_dirs: Vec<String>,

    /// C++ Standard version [default: 20] (e.g. 17, 20, 23)
    #[arg(short = 's', long = "std")]
    pub std: Option<u32>,

    /// Optimization profile: 'default' (-O2), 'o3' (-O3), or 'ofast' (-O3 -ffast-math)
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
    /// Source file to compile with debug flags and runtime sanitizers
    #[arg(value_hint = ValueHint::FilePath)]
    pub file: PathBuf,

    /// Additional include directories passed via -I (repeatable)
    #[arg(short = 'I', long = "include")]
    pub include_dirs: Vec<String>,

    /// C++ Standard version [default: 20] (e.g. 17, 20, 23)
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
    /// Target binary or problem name (e.g. 'A', '1900B', 'solution.cpp').
    /// If omitted, the most recently compiled binary is automatically selected.
    #[arg(value_hint = ValueHint::FilePath)]
    pub target: Option<String>,

    /// Force feeding input from input.txt without prompting
    #[arg(long, conflicts_with = "no_input")]
    pub input: bool,

    /// Force interactive manual terminal input, skipping input.txt
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
    /// Problem code or file name (e.g. 'A', '1899A', 'dp_knapsack')
    pub name: String,
}

impl NewArgs {
    pub fn execute(self, config: &Configuration) -> Result<()> {
        Scaffold::execute_new(&self.name, config)
    }
}

#[derive(Args)]
pub struct BundleArgs {
    /// Entry point C++ source file containing main()
    #[arg(value_hint = ValueHint::FilePath)]
    pub file: PathBuf,

    /// Destination path for the bundled output [default: <stem>_bundled.cpp]
    #[arg(short, long)]
    pub out: Option<PathBuf>,

    /// Directory paths to search when resolving custom #include headers
    #[arg(short = 'I', long = "include")]
    pub include_dirs: Vec<String>,

    /// Aggressively compress and minify identifiers using preprocessor macros
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
    /// C++ file to format
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
    /// C++ source file to compile to assembly
    #[arg(value_hint = ValueHint::FilePath)]
    pub file: PathBuf,

    /// Custom output destination path [default: <stem>.s]
    #[arg(short, long)]
    pub out: Option<PathBuf>,

    /// Keep debug symbol directives and comments in the assembly output
    #[arg(long)]
    pub debug: bool,

    /// Cross-compile to RISC-V 64 for minimal, human-readable assembly instructions
    #[arg(long)]
    pub reduced: bool,

    /// Additional include directories passed via -I (repeatable)
    #[arg(short = 'I', long = "include")]
    pub include_dirs: Vec<String>,

    /// C++ Standard version [default: 20] (e.g. 17, 20, 23)
    #[arg(short = 's', long = "std")]
    pub std: Option<u32>,

    /// Optimization profile: 'default' (-O2), 'o3' (-O3), or 'ofast' (-O3 -ffast-math)
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
    /// Port to listen on for incoming Competitive Companion HTTP requests
    #[arg(short, long, default_value_t = 10045)]
    pub port: u16,

    /// Extract short problem codes from URLs (e.g. '1900A') rather than full problem names
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
    /// Binary or problem name to profile (defaults to the newest compiled binary)
    #[arg(value_hint = ValueHint::FilePath)]
    pub target: Option<String>,

    /// Path to an input file or directory of test cases to benchmark against
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
