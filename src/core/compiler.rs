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

    fn has_sanitizers() -> bool {
        *HAS_SANITIZERS.get_or_init(|| {
            Command::new("g++")
                .args([
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

    pub fn build(file: &Path, debug: bool, include_dirs: &[PathBuf]) -> Result<PathBuf> {
        let cache_dir = Self::setup_cache()?;

        let file_stem = file.file_stem().unwrap_or_default();
        let mut out_bin = cache_dir.join(file_stem);
        out_bin.set_extension("out");

        let mut cmd = Command::new("g++");

        cmd.args([
            "-std=c++20",
            "-Wall",
            "-Wextra",
            "-Wshadow",
            "-DLOCAL",
            "-fdiagnostics-color=always",
        ]);

        for dir in include_dirs {
            cmd.arg("-I").arg(dir);
        }

        if debug {
            cmd.args(["-g", "-O1"]);
            if Self::has_sanitizers() {
                cmd.args(["-fsanitize=address,undefined", "-fno-omit-frame-pointer"]);
                Ui::meta("sanitizers", "address, undefined");
            } else {
                Ui::warn("sanitizers unavailable");
            }
        } else {
            cmd.args(["-O2", "-pipe"]);
        }

        cmd.arg(file);
        cmd.arg("-o");
        cmd.arg(&out_bin);

        println!();

        let status = cmd
            .status()
            .context("Failed to invoke g++ compiler. Is it installed?")?;

        if !status.success() {
            return Err(anyhow!("Compilation failed with status: {}", status));
        }

        Ui::ok("compiled successfully\n");
        Ok(out_bin)
    }
}
