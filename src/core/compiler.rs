use crate::utils::ui::Ui;
use anyhow::{Context, Result, anyhow};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

pub struct Compiler;

static HAS_SANITIZERS: OnceLock<bool> = OnceLock::new();

impl Compiler {
    const CACHE_DIR: &'static str = ".argo";

    fn setup_cache() -> Result<PathBuf> {
        let dir = Path::new(Self::CACHE_DIR);
        if !dir.exists() {
            fs::create_dir_all(dir).context("Failed to create .argo cache directory")?;
            fs::write(dir.join(".gitignore"), "*\n")
                .context("Failed to write .gitignore in cache")?;
        }
        Ok(dir.to_path_buf())
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

    pub fn binary_path(file: &Path) -> PathBuf {
        let cache_dir = Path::new(Self::CACHE_DIR);
        let file_stem = file.file_stem().unwrap_or_default();
        let mut out_bin = cache_dir.join(file_stem);
        out_bin.set_extension("out");
        out_bin
    }

    pub fn build(
        file: &Path,
        debug: bool,
        include_dirs: &[PathBuf],
        compiler_cmd: &str,
        log_errors_to_file: bool,
    ) -> Result<PathBuf> {
        if !file.is_file() {
            anyhow::bail!(
                "Invalid target: '{}' is a directory or does not exist.",
                file.display()
            );
        }

        let cache_dir = Self::setup_cache()?;
        let file_stem = file.file_stem().unwrap_or_default();
        let mut out_bin = cache_dir.join(file_stem);
        out_bin.set_extension("out");

        let mut parts = compiler_cmd.split_whitespace();
        let bin = parts
            .next()
            .ok_or_else(|| anyhow!("Compiler command cannot be empty"))?;

        let mut cmd = Command::new(bin);
        cmd.args(parts);

        cmd.args(["-std=c++20", "-Wall", "-Wextra", "-Wshadow", "-DLOCAL"]);

        if !log_errors_to_file {
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
                Ui::warn(format!("sanitizers unavailable for '{}'", compiler_cmd));
            }
        } else {
            cmd.args(["-O2"]);
        }

        cmd.arg(file);
        cmd.arg("-o");
        cmd.arg(&out_bin);

        if log_errors_to_file {
            let mut error_file = cache_dir.join(file_stem);
            error_file.set_extension("errors");
            let f = fs::File::create(&error_file).context("Failed to create error log file")?;
            let f_clone = f.try_clone().context("Failed to clone file handle")?;
            cmd.stdout(std::process::Stdio::from(f_clone));
            cmd.stderr(std::process::Stdio::from(f));
        }

        println!();

        let status = cmd
            .status()
            .with_context(|| format!("Failed to invoke '{}'. Is it installed?", bin))?;

        if !status.success() {
            if log_errors_to_file {
                let mut error_file = cache_dir.join(file_stem);
                error_file.set_extension("errors");
                return Err(anyhow!(
                    "Compilation failed with status: {}. See {} for details.",
                    status,
                    error_file.display()
                ));
            } else {
                return Err(anyhow!("Compilation failed with status: {}", status));
            }
        }

        Ui::ok("compiled successfully\n");
        Ok(out_bin)
    }
    pub fn resolve_test_target(query: Option<&str>) -> Result<(PathBuf, String)> {
        let cache_dir = Path::new(Self::CACHE_DIR);
        if !cache_dir.exists() {
            anyhow::bail!("No .argo cache directory found. Run `argo build` first.");
        }

        let query_str = match query {
            Some(q) if !q.trim().is_empty() => q.trim(),
            _ => {
                let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
                for entry in fs::read_dir(cache_dir)?.flatten() {
                    let p = entry.path();
                    if p.extension().is_some_and(|ext| ext == "out")
                        && let Ok(meta) = entry.metadata()
                        && let Ok(mtime) = meta.modified()
                        && newest.as_ref().is_none_or(|(max_t, _)| mtime > *max_t)
                    {
                        newest = Some((mtime, p));
                    }
                }

                let (_, bin_path) = newest.ok_or_else(|| {
                    anyhow::anyhow!("No compiled binaries found in .argo/. Run `argo build` first.")
                })?;

                let stem = bin_path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                return Ok((bin_path, format!("{stem}.cpp (auto-selected newest)")));
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
        let exact_bin = cache_dir.join(clean_stem).with_extension("out");
        if exact_bin.exists() {
            return Ok((exact_bin, format!("{clean_stem}.cpp")));
        }

        let mut scored: Vec<(f64, PathBuf, String)> = Vec::new();
        let q_lower = clean_stem.to_lowercase();

        for entry in fs::read_dir(cache_dir)?.flatten() {
            let p = entry.path();
            if p.extension().is_some_and(|ext| ext == "out") {
                let stem = p
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let score = strsim::jaro_winkler(&q_lower, &stem.to_lowercase());

                if score >= 0.65 {
                    scored.push((score, p, stem));
                }
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
            _ => anyhow::bail!("No compiled binary close to '{query_str}' found in .argo/"),
        }
    }
}
