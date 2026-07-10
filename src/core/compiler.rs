use crate::utils::ui::Ui;
use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::process::Stdio;
use std::sync::OnceLock;

pub struct Compiler;

static HAS_SANITIZERS: OnceLock<bool> = OnceLock::new();

struct CompilerDiagnostics {
    errors: usize,
    warnings: usize,
    first: Option<String>,
}

pub struct BuildArguments {
    pub file: PathBuf,
    pub include_dirs: Vec<PathBuf>,
    pub compiler_cmd: String,
    pub mode: String,
    pub debug: bool,
    pub std_version: u32,
    pub log_file: bool,
}

impl BuildArguments {
    pub fn new() -> Self {
        Self {
            file: PathBuf::new(),
            include_dirs: Vec::new(),
            compiler_cmd: "".into(),
            mode: "".into(),
            debug: false,
            std_version: 20,
            log_file: false,
        }
    }

    pub fn file(mut self, path: &Path) -> Self {
        self.file = path.to_path_buf();
        self
    }

    pub fn debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    pub fn includes(mut self, paths: &[PathBuf]) -> Self {
        self.include_dirs = paths.to_vec();
        self
    }

    pub fn cmd(mut self, cmd: impl Into<String>) -> Self {
        self.compiler_cmd = cmd.into();
        self
    }

    pub fn std(mut self, version: u32) -> Self {
        self.std_version = version;
        self
    }

    pub fn log(mut self, value: bool) -> Self {
        self.log_file = value;
        self
    }

    pub fn mode(mut self, mode: impl Into<String>) -> Self {
        self.mode = mode.into();
        self
    }
}

impl Compiler {
    const CACHE_DIR: &'static str = ".argo";

    const WARNING_FLAGS: [&str; 4] = ["-Wall", "-Wextra", "-Wshadow", "-DLOCAL"];
    const SANITIZER_FLAGS: [&str; 2] = ["-fsanitize=address,undefined", "-fno-omit-frame-pointer"];
    const SANITIZER_PROBE_FLAGS: [&str; 6] = [
        "-fsanitize=address,undefined",
        "-x",
        "c++",
        "-",
        "-o",
        "/dev/null",
    ];
    const DEBUG_FLAGS: [&str; 2] = ["-g", "-O1"];
    const OPTIMISED_DEFAULT: &str = "-O2";
    const OPTIMISED_MAXIMUM: &str = "-O3";
    const OPTIMISED_BREAKING: &str = "-Ofast";
    const CROSS_COMPILER_CANDIDATES: [&str; 3] = [
        "riscv64-buildroot-linux-gnu-g++",
        "riscv64-linux-gnu-g++",
        "riscv64-unknown-linux-gnu-g++",
    ];

    fn setup_cache(file: &Path) -> Result<PathBuf> {
        let parent = Self::parent_or_default(file);
        let directory = parent.join(Self::CACHE_DIR);
        if !directory.exists() {
            match Self::create_directory(&directory) {
                Ok(()) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(directory)
    }

    pub fn binary_path(file: &Path) -> PathBuf {
        let parent = Self::parent_or_default(file);
        let cache_dir = parent.join(Self::CACHE_DIR);
        let file_stem = file.file_stem().unwrap_or_default();
        let mut out_bin = cache_dir.join(file_stem);
        out_bin.set_extension("out");
        out_bin
    }

    pub fn cross_compiler() -> String {
        if let Ok(path) = std::env::var("PATH") {
            for directory in std::env::split_paths(&path) {
                for candidate in Self::CROSS_COMPILER_CANDIDATES {
                    let target = directory.join(candidate);
                    if target.exists() {
                        return candidate.to_string();
                    }
                }
            }
        }
        "riscv64-linux-gnu-g++".to_string()
    }

    fn has_sanitizers(compiler_cmd: &str) -> bool {
        let init = || {
            let mut command = match Self::sanitizer_probe(compiler_cmd) {
                Ok(command) => command,
                Err(_) => return false,
            };
            match command.status() {
                Ok(status) => status.success(),
                Err(_) => false,
            }
        };
        *HAS_SANITIZERS.get_or_init(init)
    }

    fn create_base_cmd(args: &BuildArguments, color_diagnostics: bool) -> Result<Command> {
        let mut command = Self::compiler_command(&args.compiler_cmd)?;
        Self::common_flags(&mut command, args.std_version, color_diagnostics);
        Self::include_dirs(&mut command, &args.include_dirs);
        if args.debug {
            Self::configure_debug(&mut command, &args.compiler_cmd);
        } else {
            Self::configure_release(&mut command, &args.mode);
        }
        Ok(command)
    }

    pub fn build(args: BuildArguments) -> Result<PathBuf> {
        Self::validate_target(&args.file)?;
        let cache_directory = Self::setup_cache(&args.file)?;
        let out_binary = Self::binary_path(&args.file);

        let mut command = Self::create_base_cmd(&args, !args.log_file)?;

        println!();
        command.arg(&args.file).arg("-o").arg(&out_binary);

        if args.log_file {
            let target = args.file.file_stem().unwrap_or_default();
            let mut error_file = cache_directory.join(target);
            error_file.set_extension("errors");
            Self::logged_execution(&mut command, &args.compiler_cmd, &error_file)?;
        } else {
            Self::standard_execution(&mut command, &args.compiler_cmd)?;
        }
        Ok(out_binary)
    }

    pub fn peek(args: BuildArguments, out: Option<&Path>) -> Result<PathBuf> {
        Self::validate_target(&args.file)?;

        let output_file = match out {
            Some(path) => path.to_path_buf(),
            None => args.file.with_extension("s"),
        };

        let mut command = Self::create_base_cmd(&args, true)?;

        println!();
        command
            .arg("-S")
            .arg(&args.file)
            .arg("-o")
            .arg(&output_file);

        Self::execute_command(&mut command, &args.compiler_cmd)?;
        Ui::ok(format!(
            "assembly output written to {}",
            output_file.display()
        ));
        Ok(output_file)
    }

    pub fn resolve_test_target(query: Option<&str>) -> Result<(PathBuf, String)> {
        let candidates = FuzzyMatching::find_candidates()?;

        if candidates.is_empty() {
            anyhow::bail!("No compiled binaries found in local tree. Run 'argo build' first.");
        }

        let query_str = match query {
            Some(query) if !query.trim().is_empty() => query.trim(),
            _ => {
                let best = candidates
                    .into_iter()
                    .max_by_key(|candidate| candidate.mtime)
                    .unwrap();
                return Ok((
                    best.path,
                    format!("{}.cpp (auto-selected newest)", best.stem),
                ));
            }
        };

        let as_path = Path::new(query_str);
        if as_path.exists() {
            let binary = Self::binary_path(as_path);
            if binary.exists() {
                return Ok((binary, query_str.to_string()));
            }
        }

        let clean_stem = query_str.strip_suffix(".cpp").unwrap_or(query_str);
        if let Some(matched) = candidates
            .iter()
            .find(|candidate| candidate.stem == clean_stem)
        {
            return Ok((matched.path.clone(), format!("{}.cpp", matched.stem)));
        }

        FuzzyMatching::fuzzy_match(clean_stem, candidates)
    }

    fn create_directory(directory: &PathBuf) -> Result<()> {
        let create_result = fs::create_dir_all(directory);
        if create_result.is_err() {
            let error_str = String::from("Failed to create .argo cache directory");
            return Err(anyhow::anyhow!(error_str));
        }

        let gitignore = directory.join(".gitignore");
        let write_result = fs::write(gitignore, "*\n");
        if write_result.is_err() {
            let error_str = String::from("Failed to write .gitignore in cache.");
            return Err(anyhow::anyhow!(error_str));
        }

        Ok(())
    }

    fn parent_or_default(file: &Path) -> &Path {
        match file.parent() {
            Some(parent) => parent,
            None => Path::new("."),
        }
    }

    fn sanitizer_probe(compiler_cmd: &str) -> Result<Command> {
        let mut command = Self::compiler_command(compiler_cmd)?;
        command.args(Self::SANITIZER_PROBE_FLAGS);
        command.stdin(Stdio::null());
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());
        Ok(command)
    }

    fn common_flags(command: &mut Command, std_version: u32, color_diagnostics: bool) {
        command.arg(format!("-std=c++{}", std_version));
        command.args(Self::WARNING_FLAGS);

        if color_diagnostics {
            command.arg("-fdiagnostics-color=always");
        }
    }

    fn include_dirs(command: &mut Command, include_dirs: &[PathBuf]) {
        for directory in include_dirs {
            command.arg("-I").arg(directory);
        }
    }

    fn configure_debug(command: &mut Command, compiler_cmd: &str) {
        command.args(Self::DEBUG_FLAGS);

        if Self::has_sanitizers(compiler_cmd) {
            command.args(Self::SANITIZER_FLAGS);
            Ui::meta("sanitizers", "address, undefined");
        } else {
            Ui::meta("sanitizers", "unavailable");
        }
    }

    fn configure_release(command: &mut Command, mode: &str) {
        match mode {
            "ofast" => {
                command.arg(Self::OPTIMISED_BREAKING);
                Ui::meta("mode", Self::OPTIMISED_BREAKING);
            }
            "o3" => {
                command.arg(Self::OPTIMISED_MAXIMUM);
                Ui::meta("mode", Self::OPTIMISED_MAXIMUM);
            }
            _ => {
                command.arg(Self::OPTIMISED_DEFAULT);
                Ui::meta("mode", Self::OPTIMISED_DEFAULT);
            }
        }
    }

    fn compiler_binary(compiler_cmd: &str) -> Result<&str> {
        match compiler_cmd.split_whitespace().next() {
            Some(command) => Ok(command),
            None => {
                let error_string = String::from("Compiler command cannot be empty");
                Err(anyhow::anyhow!(error_string))
            }
        }
    }

    fn compiler_command(compiler_cmd: &str) -> Result<Command> {
        let compiler = Self::compiler_binary(compiler_cmd)?;
        let mut command = Command::new(compiler);
        let args = compiler_cmd.split_whitespace().skip(1);
        command.args(args);
        Ok(command)
    }

    fn logged_execution(
        command: &mut Command,
        compiler_cmd: &str,
        error_file: &Path,
    ) -> Result<()> {
        let output = match command.output() {
            Ok(output) => output,
            Err(_) => {
                let error_string = format!(
                    "Failed to invoke '{}'. Is it installed?",
                    Self::compiler_binary(compiler_cmd)?
                );
                return Err(anyhow::anyhow!(error_string));
            }
        };

        let mut combined_output = Vec::new();
        combined_output.extend_from_slice(&output.stdout);
        combined_output.extend_from_slice(&output.stderr);
        fs::write(error_file, combined_output).context("Failed to write error log file")?;

        let stderr_str = String::from_utf8_lossy(&output.stderr);
        let diagnostics = Self::parse_diagnostics(&stderr_str);

        if !output.status.success() {
            let mut err_str = format!(
                "compilation failed: {} errors, {} warnings",
                diagnostics.errors, diagnostics.warnings
            );

            if let Some(err_msg) = diagnostics.first {
                let colorised_msg = err_msg.trim().red();
                let dimmed_arrow = "↳".dimmed();
                err_str.push_str(&format!("\n {} {}", dimmed_arrow, colorised_msg));
            }

            let full_log = format!("full log saved to {}", error_file.display()).dimmed();
            err_str.push_str(&format!("\n {} {}", "".cyan(), full_log));

            Err(anyhow::anyhow!(err_str))
        } else if diagnostics.warnings > 0 {
            Ui::warn(format!(
                "compiled successfully with {} warnings",
                diagnostics.warnings
            ));
            Ui::info(format!("details saved to {}", error_file.display()));
            Ok(())
        } else {
            Ui::ok("compiled successfully");
            Ok(())
        }
    }

    fn standard_execution(command: &mut Command, compiler_cmd: &str) -> Result<()> {
        Self::execute_command(command, compiler_cmd)?;
        Ui::ok("compiled successfully");
        Ok(())
    }

    fn parse_diagnostics(stderr: &str) -> CompilerDiagnostics {
        let mut errors: usize = 0;
        let mut warnings: usize = 0;
        let mut first = None;

        for line in stderr.lines() {
            let lowered = line.to_lowercase();
            if lowered.contains("error:") || lowered.contains("fatal error:") {
                errors += 1;
                if first.is_none() {
                    let line_string = line.trim().to_string();
                    first = Some(line_string);
                }
            } else if lowered.contains("warning:") {
                warnings += 1;
            }
        }

        CompilerDiagnostics {
            errors,
            warnings,
            first,
        }
    }

    fn validate_target(file: &Path) -> Result<()> {
        if !file.is_file() {
            anyhow::bail!(
                "Invalid target: '{}' is a directory or does not exist",
                file.display()
            );
        }
        Ok(())
    }

    fn execute_command(command: &mut Command, compiler_cmd: &str) -> Result<()> {
        let status = command.status().map_err(|_| {
            let binary = Self::compiler_binary(compiler_cmd).unwrap_or_default();
            anyhow::anyhow!("Failed to invoke '{binary}'. Is it installed?")
        })?;

        if !status.success() {
            return Err(anyhow::anyhow!("compilation failed ({status})"));
        }

        Ok(())
    }
}

struct MatchCandidate {
    mtime: std::time::SystemTime,
    path: PathBuf,
    stem: String,
}

struct FuzzyMatching;

impl FuzzyMatching {
    const CACHE_DIR: &'static str = ".argo";
    const JARO_WRINKLER_THRESHOLD: f64 = 0.65;
    const UPPER_JARO_WRINKLER_THRESHOLD: f64 = 0.72;
    const RUNNER_UP_DISAMBIGUATION: f64 = 0.05;

    pub fn find_candidates() -> Result<Vec<MatchCandidate>> {
        let current_directory = std::env::current_dir()?;
        let mut candidates: Vec<MatchCandidate> = Vec::new();

        let mut check_directory = |directory: &Path| -> Result<()> {
            let argo_directory = directory.join(Self::CACHE_DIR);
            if argo_directory.exists() && argo_directory.is_dir() {
                let entries = fs::read_dir(&argo_directory).map_err(|_| {
                    let error_string =
                        format!("Could not read entries from {}", argo_directory.display());
                    anyhow::anyhow!(error_string)
                })?;

                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().is_some_and(|ext| ext == "out")
                        && let Ok(meta) = entry.metadata()
                        && let Ok(mtime) = meta.modified()
                    {
                        let stem = path
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        candidates.push(MatchCandidate { mtime, path, stem });
                    }
                }
            }
            Ok(())
        };

        check_directory(&current_directory)?;
        if let Ok(entries) = fs::read_dir(&current_directory) {
            for entry in entries.flatten() {
                if entry.path().is_dir() && entry.file_name() != Self::CACHE_DIR {
                    check_directory(&entry.path())?;
                }
            }
        };

        Ok(candidates)
    }

    pub fn fuzzy_match(
        query_stem: &str,
        candidates: Vec<MatchCandidate>,
    ) -> Result<(PathBuf, String)> {
        let mut scored = Vec::new();
        let query_lowered = query_stem.to_lowercase();

        for candidate in candidates {
            let score = strsim::jaro_winkler(&query_lowered, &candidate.stem.to_lowercase());
            if score >= Self::JARO_WRINKLER_THRESHOLD {
                scored.push((score, candidate));
            }
        }

        let compare = |first: &(f64, MatchCandidate), second: &(f64, MatchCandidate)| {
            second
                .0
                .partial_cmp(&first.0)
                .unwrap_or(std::cmp::Ordering::Equal)
        };

        scored.sort_by(compare);

        match scored.as_slice() {
            [(best_score, best_candidate)]
                if *best_score >= Self::UPPER_JARO_WRINKLER_THRESHOLD =>
            {
                let runner_up = scored.get(1);

                if let Some(runner_up) = runner_up {
                    let within = (best_score - runner_up.0).abs() < Self::RUNNER_UP_DISAMBIGUATION;

                    if within {
                        anyhow::bail!(
                            "Ambiguous target '{query_stem}'. Did you mean '{}.cpp' ({:.0}%) or '{}.cpp' ({:.0}%)?",
                            best_candidate.stem,
                            best_score * 100.0,
                            runner_up.1.stem,
                            runner_up.0 * 100.0,
                        );
                    }
                }

                Ok((
                    best_candidate.path.clone(),
                    format!(
                        "{}.cpp (jaro-winkler {:.0}%)",
                        best_candidate.stem,
                        best_score * 100.0
                    ),
                ))
            }
            _ => anyhow::bail!("No compiled binary close to '{query_stem}' found."),
        }
    }
}
