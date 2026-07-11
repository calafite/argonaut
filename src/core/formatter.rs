use crate::utils::paths::PathUtilities;
use crate::utils::ui::Ui;
use anyhow::{Context, Result};
use colored::Colorize;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub struct Formatter;

impl Formatter {
    pub fn execute_format(file: &Path) -> Result<()> {
        Ui::section("Code Formatter");
        Self::format(file)
    }

    pub fn format(file: &Path) -> Result<()> {
        let (absolute_file, source) = Self::prepare(file)?;
        let file_name = absolute_file
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();

        let (working_directory, using_central) = Self::resolve_config(&absolute_file)?;

        let output = match Self::clang_format(&working_directory, &file_name, &source) {
            Ok(output) => output,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Self::installation_warning();
                return Ok(());
            }
            Err(error) => return Err(error).context("Failed to run clang-format"),
        };

        if output.status.success() {
            std::fs::write(&absolute_file, output.stdout)
                .context("Failed to write formatted code back to file")?;
            let configuration_type = if using_central { "central" } else { "local" };
            Ui::ok(format!(
                "formatted {} (using {} config)",
                file.display(),
                configuration_type
            ));
        } else {
            let err_str = Self::format_error(file, &output.stderr);
            return Err(anyhow::anyhow!(err_str));
        }

        Ok(())
    }

    fn prepare(file: &Path) -> Result<(PathBuf, Vec<u8>)> {
        let absolute_file = match file.canonicalize() {
            Ok(file) => file,
            Err(_) => {
                let error_string = String::from("File not found or invalid path");
                return Err(anyhow::anyhow!(error_string));
            }
        };

        if !absolute_file.is_file() {
            anyhow::bail!("Invalid target: '{}' is a directory.", file.display());
        }

        let source = match std::fs::read(&absolute_file) {
            Ok(content) => content,
            Err(_) => {
                let error_string = String::from("Failed to read source file");
                return Err(anyhow::anyhow!(error_string));
            }
        };

        Ok((absolute_file, source))
    }

    fn resolve_config(file: &Path) -> Result<(PathBuf, bool)> {
        if let Some(working_directory) = Self::find_config(file) {
            Ok((working_directory, false))
        } else {
            let central_directory = Self::central_config()?;
            Ok((central_directory, true))
        }
    }

    fn find_config(file: &Path) -> Option<PathBuf> {
        let mut current = Some(PathUtilities::parent_or_default(file));
        while let Some(directory) = current {
            let exists = directory.join(".clang-format").exists()
                || directory.join("_clang-format").exists();

            if exists {
                let path_buf = PathUtilities::parent_or_default(file).to_path_buf();
                return Some(path_buf);
            }
            current = directory.parent();
        }
        None
    }

    fn central_config() -> Result<PathBuf> {
        let project_directories = match PathUtilities::project_dirs() {
            Some(dirs) => dirs,
            None => {
                let error_string = String::from("Could not determine user configuration directory");
                return Err(anyhow::anyhow!(error_string));
            }
        };

        let config_directory = project_directories.config_dir().to_path_buf();
        if !config_directory.exists() {
            PathUtilities::create_config(&config_directory)?;
        }

        let central_configuration = config_directory.join(".clang-format");
        if !central_configuration.exists() {
            let write_str = include_str!(".default.clang-format");
            let result = std::fs::write(&central_configuration, write_str);
            if result.is_err() {
                anyhow::bail!("Failed to write central .clang-format");
            }
            Ui::info(format!(
                "Initialized optimised configuration at {}",
                central_configuration.display()
            ));
        }

        Ok(config_directory)
    }

    fn clang_format(
        working_directory: &Path,
        file_name: &str,
        source: &[u8],
    ) -> std::io::Result<std::process::Output> {
        let mut command = Self::build_command(working_directory, file_name);
        Self::execute(&mut command, source)
    }

    fn build_command(working_directory: &Path, file_name: &str) -> Command {
        let mut command = Command::new("clang-format");
        command
            .current_dir(working_directory)
            .arg("--style-file")
            .arg(format!("--asume-filename={}", file_name))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    fn execute(command: &mut Command, stdin_data: &[u8]) -> std::io::Result<std::process::Output> {
        let mut child = command.spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(stdin_data)?;
        }
        child.wait_with_output()
    }

    fn installation_warning() {
        Ui::warn("clang-format is not installed or not in PATH.");
        Ui::info("Ubuntu/Debian: sudo apt install clang-format");
        Ui::info("Fedora: sudo dnf install clang-format");
        Ui::info("macOS: brew install clang-format");
    }

    fn format_error(file: &Path, stderr: &[u8]) -> String {
        let mut err_str = format!("formatter failed on {}", file.display());
        let err_msg = String::from_utf8_lossy(stderr);
        if !err_msg.trim().is_empty() {
            for line in err_msg.lines() {
                err_str.push_str(&format!("\n  {}  {}", "↳".dimmed(), line.trim().red()));
            }
        }
        err_str
    }
}
