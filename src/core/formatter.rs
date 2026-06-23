use crate::utils::ui::Ui;
use anyhow::{Context, Result};
use colored::Colorize;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Formatter;

impl Formatter {
    pub fn format(file: &Path) -> Result<()> {
        let abs_file = file
            .canonicalize()
            .context("File not found or invalid path")?;

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

        let config_dir = Self::find_or_init_config(&abs_file)?;

        let output = Command::new("clang-format")
            .current_dir(&config_dir)
            .arg("--style=file")
            .arg("-i")
            .arg(&abs_file)
            .output()
            .context("Failed to invoke clang-format process")?;

        if output.status.success() {
            Ui::ok(format!("Formatted {}", file.display()));
        } else {
            Ui::fail(format!("Formatter failed on {}", file.display()));
            let err_msg = String::from_utf8_lossy(&output.stderr);
            if !err_msg.trim().is_empty() {
                for line in err_msg.lines() {
                    println!("  {} {}", "↳".dimmed(), line.trim().red());
                }
            }
        }

        Ok(())
    }

    fn find_or_init_config(file: &Path) -> Result<PathBuf> {
        let parent_dir = file.parent().unwrap_or_else(|| Path::new("."));
        let mut current_dir = parent_dir.to_path_buf();

        loop {
            if current_dir.join(".clang-format").exists()
                || current_dir.join("_clang-format").exists()
            {
                return Ok(current_dir);
            }
            if !current_dir.pop() {
                break;
            }
        }

        let target_dir = parent_dir.to_path_buf();
        let config_path = target_dir.join(".clang-format");

        std::fs::write(&config_path, include_str!(".default.clang-format"))
            .context("Failed to write default .clang-format")?;

        Ui::info(format!(
            "Initialized CP-optimized config at {}",
            config_path.display()
        ));

        Ok(target_dir)
    }
}
