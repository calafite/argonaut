use crate::utils::ui::Ui;
use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

pub struct Compiler;

static HAS_SANITIZERS: OnceLock<bool> = OnceLock::new();

impl Compiler {
    const CACHE_DIR: &'static str = ".argo";

    fn setup_cache(file: &Path) -> Result<PathBuf> {
        let parent = file.parent().unwrap_or_else(|| Path::new("."));
        let dir = parent.join(Self::CACHE_DIR);
        if !dir.exists() {
            fs::create_dir_all(&dir).context("Failed to create .argo cache directory")?;
            fs::write(dir.join(".gitignore"), "*\n")
                .context("Failed to write .gitignore in cache")?;
        }
        Ok(dir)
    }

    pub fn binary_path(file: &Path) -> PathBuf {
        let parent = file.parent().unwrap_or_else(|| Path::new("."));
        let cache_dir = parent.join(Self::CACHE_DIR);
        let file_stem = file.file_stem().unwrap_or_default();
        let mut out_bin = cache_dir.join(file_stem);
        out_bin.set_extension("out");
        out_bin
    }

    fn has_sanitizers(compiler_cmd: &str) -> bool {
        *HAS_SANITIZERS.get_or_init(|| {
            let mut parts = compiler_cmd.split_whitespace();
            let bin = parts.next().unwrap_or("g++");

            let mut cmd = Command::new(bin);
            cmd.args(parts);
            cmd.args([
                "-fsanitize=address,undefined",
                "-x",
                "c++",
                "-",
                "-o",
                "/dev/null",
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        })
    }

    fn create_base_cmd(
        compiler_cmd: &str,
        std_version: u32,
        debug: bool,
        include_dirs: &[PathBuf],
        color_diagnostics: bool,
        mode: String,
    ) -> Result<Command> {
        let mut parts = compiler_cmd.split_whitespace();
        let bin = parts
            .next()
            .ok_or_else(|| anyhow!("Compiler command cannot be empty"))?;

        let mut cmd = Command::new(bin);
        cmd.args(parts);

        cmd.arg(format!("-std=c++{std_version}"));
        cmd.args(["-Wall", "-Wextra", "-Wshadow", "-DLOCAL"]);

        if color_diagnostics {
            cmd.arg("-fdiagnostics-color=always");
        }

        for dir in include_dirs {
            cmd.arg("-I").arg(dir);
        }

        if debug {
            cmd.args(["-g", "-O1"]);
            if Self::has_sanitizers(compiler_cmd) {
                cmd.args(["-fsanitize=address,undefined", "-fno-omit-frame-pointer"]);
                Ui::meta("sanitizers", "address, undefined");
            } else {
                Ui::meta("sanitizers", "unavailable");
            }
        } else {
            if mode == "o3" {
                cmd.args(["-O3"]);
                Ui::meta("mode", "O3");
            } else {
                cmd.args(["-O2"]);
                Ui::meta("mode", "O2");
            }
        }

        Ok(cmd)
    }

    pub fn build(
        file: &Path,
        debug: bool,
        include_dirs: &[PathBuf],
        compiler_cmd: &str,
        std_version: u32,
        log_file: bool,
        mode: String,
    ) -> Result<PathBuf> {
        if !file.is_file() {
            anyhow::bail!(
                "Invalid target: '{}' is a directory or does not exist.",
                file.display()
            );
        }

        let cache_dir = Self::setup_cache(file)?;
        let file_stem = file.file_stem().unwrap_or_default();
        let mut out_bin = cache_dir.join(file_stem);
        out_bin.set_extension("out");

        let mut cmd = Self::create_base_cmd(
            compiler_cmd,
            std_version,
            debug,
            include_dirs,
            !log_file,
            mode.clone(),
        )?;

        println!();

        cmd.arg(file);
        cmd.arg("-o");
        cmd.arg(&out_bin);

        if log_file {
            let output = cmd.output().with_context(|| {
                format!(
                    "Failed to invoke '{}'. Is it installed?",
                    compiler_cmd.split_whitespace().next().unwrap_or("")
                )
            })?;

            let mut error_file = cache_dir.join(file_stem);
            error_file.set_extension("errors");

            let stderr_str = String::from_utf8_lossy(&output.stderr);
            let mut combined_out = Vec::new();
            combined_out.extend_from_slice(&output.stdout);
            combined_out.extend_from_slice(&output.stderr);
            fs::write(&error_file, combined_out).context("Failed to write error log file")?;

            let mut error_count = 0;
            let mut warning_count = 0;
            let mut first_error = None;

            for line in stderr_str.lines() {
                let lower = line.to_lowercase();
                if lower.contains("error:") || lower.contains("fatal error:") {
                    error_count += 1;
                    if first_error.is_none() {
                        first_error = Some(line.trim().to_string());
                    }
                } else if lower.contains("warning:") {
                    warning_count += 1;
                }
            }

            if !output.status.success() {
                let mut err_str = format!(
                    "compilation failed: {} errors, {} warnings",
                    error_count, warning_count
                );
                if let Some(err_msg) = first_error {
                    err_str.push_str(&format!("\n  {}  {}", "↳".dimmed(), err_msg.trim().red()));
                }
                err_str.push_str(&format!(
                    "\n  {}  {}",
                    "".cyan(),
                    format!("full log saved to {}", error_file.display()).dimmed()
                ));

                return Err(anyhow::anyhow!(err_str));
            } else if warning_count > 0 {
                Ui::warn(format!(
                    "compiled successfully with {} warnings",
                    warning_count
                ));
                Ui::info(format!("details saved to {}", error_file.display()));
            } else {
                Ui::ok("compiled successfully");
            }
        } else {
            let status = cmd.status().with_context(|| {
                format!(
                    "Failed to invoke '{}'. Is it installed?",
                    compiler_cmd.split_whitespace().next().unwrap_or("")
                )
            })?;

            if !status.success() {
                return Err(anyhow::anyhow!("compilation failed ({})", status));
            }
            Ui::ok("compiled successfully");
        }

        Ok(out_bin)
    }

    pub fn peek(
        file: &Path,
        out: Option<&Path>,
        debug: bool,
        include_dirs: &[PathBuf],
        compiler_cmd: &str,
        std_version: u32,
        mode: String,
    ) -> Result<PathBuf> {
        if !file.is_file() {
            anyhow::bail!(
                "Invalid target: '{}' is a directory or does not exist.",
                file.display()
            );
        }

        let out_file = match out {
            Some(p) => p.to_path_buf(),
            None => {
                let mut p = file.to_path_buf();
                p.set_extension("s");
                p
            }
        };

        let mut cmd =
            Self::create_base_cmd(compiler_cmd, std_version, debug, include_dirs, true, mode)?;

        println!();

        cmd.arg("-S");
        cmd.arg(file);
        cmd.arg("-o");
        cmd.arg(&out_file);

        let status = cmd.status().with_context(|| {
            format!(
                "Failed to invoke '{}'. Is it installed?",
                compiler_cmd.split_whitespace().next().unwrap_or("")
            )
        })?;

        if !status.success() {
            return Err(anyhow::anyhow!("compilation failed ({})", status));
        }
        Ui::ok(format!("assembly written to {}", out_file.display()));

        Ok(out_file)
    }

    pub fn resolve_test_target(query: Option<&str>) -> Result<(PathBuf, String)> {
        let current_dir = std::env::current_dir()?;
        let mut candidates = Vec::new();

        let mut check_dir = |dir: &Path| {
            let argo_dir = dir.join(Self::CACHE_DIR);
            if argo_dir.exists()
                && argo_dir.is_dir()
                && let Ok(entries) = fs::read_dir(argo_dir)
            {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.extension().is_some_and(|ext| ext == "out")
                        && let Ok(meta) = entry.metadata()
                        && let Ok(mtime) = meta.modified()
                    {
                        let stem = p
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        candidates.push((mtime, p, stem));
                    }
                }
            }
        };

        check_dir(&current_dir);
        if let Ok(entries) = fs::read_dir(&current_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() && entry.file_name() != Self::CACHE_DIR {
                    check_dir(&entry.path());
                }
            }
        }

        if candidates.is_empty() {
            anyhow::bail!("No compiled binaries found in local tree. Run `argo build` first.");
        }

        let query_str = match query {
            Some(q) if !q.trim().is_empty() => q.trim(),
            _ => {
                let best = candidates
                    .into_iter()
                    .max_by_key(|(mtime, _, _)| *mtime)
                    .unwrap();
                return Ok((best.1, format!("{}.cpp (auto-selected newest)", best.2)));
            }
        };

        let as_path = Path::new(query_str);
        if as_path.exists() {
            let bin = Self::binary_path(as_path);
            if bin.exists() {
                return Ok((bin, query_str.to_string()));
            }
        }

        let clean_stem = query_str.strip_suffix(".cpp").unwrap_or(query_str);

        for (_, p, stem) in &candidates {
            if stem == clean_stem {
                return Ok((p.clone(), format!("{}.cpp", stem)));
            }
        }

        let mut scored: Vec<(f64, PathBuf, String)> = Vec::new();
        let q_lower = clean_stem.to_lowercase();

        for (_, p, stem) in candidates {
            let score = strsim::jaro_winkler(&q_lower, &stem.to_lowercase());
            if score >= 0.65 {
                scored.push((score, p, stem));
            }
        }

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        match scored.as_slice() {
            [(best_score, bin, stem), ..] if *best_score >= 0.72 => {
                if let Some((runner_up_score, _, runner_up_stem)) = scored.get(1)
                    && (best_score - runner_up_score).abs() < 0.05
                {
                    anyhow::bail!(
                        "Ambiguous target '{query_str}'. Did you mean '{stem}.cpp' ({:.0}%) or '{runner_up_stem}.cpp' ({:.0}%)?",
                        best_score * 100.0,
                        runner_up_score * 100.0
                    );
                }

                Ok((
                    bin.clone(),
                    format!("{stem}.cpp (jaro-winkler {:.0}%)", best_score * 100.0),
                ))
            }
            _ => anyhow::bail!("No compiled binary close to '{query_str}' found."),
        }
    }
}
