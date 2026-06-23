use crate::utils::ui::Ui;
use anyhow::{Context, Result};
use colored::Colorize;
use directories::ProjectDirs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub struct Formatter;

impl Formatter {
    pub fn format(file: &Path) -> Result<()> {
        let abs_file = file
            .canonicalize()
            .context("File not found or invalid path")?;

        if !abs_file.is_file() {
            anyhow::bail!("Invalid target: '{}' is a directory.", file.display());
        }

        if Command::new("clang-format")
            .arg("--version")
            .output()
            .is_err()
        {
            Ui::warn("clang-format is not installed or not in PATH.");
            Ui::info("Ubuntu/Debian: sudo apt install clang-format");
            Ui::info("Fedora: sudo dnf install clang-format");
            Ui::info("macOS: brew install clang-format");
            return Ok(());
        }

        let (working_dir, using_central) = Self::resolve_config_environment(&abs_file)?;

        let source = std::fs::read(&abs_file).context("Failed to read source file")?;

        let file_name = abs_file.file_name().unwrap_or_default().to_string_lossy();

        let mut child = Command::new("clang-format")
            .current_dir(&working_dir)
            .arg("--style=file")
            .arg(format!("--assume-filename={}", file_name))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to invoke clang-format process")?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(&source)
                .context("Failed to pipe source to clang-format")?;
        }

        let output = child
            .wait_with_output()
            .context("Failed to await clang-format")?;

        if output.status.success() {
            std::fs::write(&abs_file, output.stdout)
                .context("Failed to write formatted code back to file")?;

            if using_central {
                Ui::ok(format!(
                    "formatted {} (using central config)",
                    file.display()
                ));
            } else {
                Ui::ok(format!("formatted {} (using local config)", file.display()));
            }
        } else {
            let mut err_str = format!("formatter failed on {}", file.display());
            let err_msg = String::from_utf8_lossy(&output.stderr);
            if !err_msg.trim().is_empty() {
                for line in err_msg.lines() {
                    err_str.push_str(&format!("\n  {}  {}", "↳".dimmed(), line.trim().red()));
                }
            }
            return Err(anyhow::anyhow!(err_str));
        }

        Ok(())
    }

    fn resolve_config_environment(file: &Path) -> Result<(PathBuf, bool)> {
        let mut check_dir = file
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let target_dir = check_dir.clone();

        loop {
            if check_dir.join(".clang-format").exists() || check_dir.join("_clang-format").exists()
            {
                return Ok((target_dir, false));
            }
            if !check_dir.pop() {
                break;
            }
        }

        let proj_dirs = ProjectDirs::from("", "", "argo")
            .context("Could not determine user configuration directory")?;

        let config_dir = proj_dirs.config_dir().to_path_buf();
        if !config_dir.exists() {
            std::fs::create_dir_all(&config_dir)
                .context("Failed to create argo config directory")?;
        }

        let central_config = config_dir.join(".clang-format");
        if !central_config.exists() {
            std::fs::write(&central_config, include_str!(".default.clang-format"))
                .context("Failed to write central .clang-format")?;
            Ui::info(format!(
                "Initialized central CP-optimized config at {}",
                central_config.display()
            ));
        }

        Ok((config_dir, true))
    }
}
