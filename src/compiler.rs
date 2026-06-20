use crate::ui::Ui;
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
        let file_stem = file.file_stem().unwrap_or_default().to_string_lossy();
        let out_bin = cache_dir.join(format!("{}.out", file_stem));

        let include_args: Vec<String> = include_dirs
            .iter()
            .flat_map(|dir| vec!["-I".to_string(), dir.to_string_lossy().into_owned()])
            .collect();

        let mut args = vec![
            "-std=c++20",
            "-Wall",
            "-Wextra",
            "-Wshadow",
            "-DLOCAL",
            "-fdiagnostics-color=always",
        ];

        let include_args_refs: Vec<&str> = include_args.iter().map(|s| s.as_str()).collect();
        args.extend(include_args_refs);

        if debug {
            args.extend(&["-g", "-O1"]);
            if Self::has_sanitizers() {
                args.extend(&["-fsanitize=address,undefined", "-fno-omit-frame-pointer"]);
                Ui::meta("sanitizers", "address, undefined");
            } else {
                Ui::warn("sanitizers unavailable");
            }
        } else {
            args.extend(&["-O2", "-pipe"]);
        }

        args.push(file.to_str().unwrap());
        args.push("-o");
        args.push(out_bin.to_str().unwrap());

        println!();

        let status = Command::new("g++")
            .args(&args)
            .status()
            .context("Failed to invoke g++ compiler. Is it installed?")?;

        if !status.success() {
            return Err(anyhow!("Compilation failed with status: {}", status));
        }

        Ui::ok("compiled successfully\n");
        Ok(out_bin)
    }
}
