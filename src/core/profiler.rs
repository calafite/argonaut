use crate::core::compiler::Compiler;
use crate::utils::paths::PathUtilities;
use crate::utils::ui::Ui;
use anyhow::Result;
use colored::Colorize;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub struct Profiler;

impl Profiler {
    pub fn execute_profile(
        target: Option<&str>,
        input_override: Option<&str>,
        config: &crate::config::settings::Configuration,
    ) -> Result<()> {
        let (binary, display) = Compiler::resolve_test_target(target)?;
        Ui::section("Profiler Configuration");
        Ui::meta("target", display);

        let argo_dir = PathUtilities::parent_or_default(&binary).to_path_buf();
        let project_dir = argo_dir.parent().unwrap_or_else(|| Path::new("."));
        let stem = binary.file_stem().unwrap_or_default().to_string_lossy();
        let source_file = project_dir.join(format!("{}.cpp", stem));

        let test_directory = argo_dir.join("tests").join(stem.as_ref());
        let mut input_path = None;

        if let Some(ui) = input_override {
            let path_buf = PathBuf::from(ui);
            if path_buf.exists() {
                input_path = Some(path_buf);
            }
        } else {
            if test_directory.exists() && test_directory.is_dir() {
                input_path = Some(test_directory);
            } else {
                let default_in = project_dir.join("input.txt");
                if default_in.exists() {
                    input_path = Some(default_in);
                }
            }
        }

        let mut asm_file = None;
        let local_asm = project_dir.join(format!("{}.s", stem));
        let cached_asm = argo_dir.join(format!("{}.s", stem));

        if local_asm.exists() {
            asm_file = Some(local_asm);
        } else if cached_asm.exists() {
            asm_file = Some(cached_asm);
        } else if source_file.exists() {
            Ui::warn(format!(
                "No assembly file found. Run 'argo peek {}' first to enable microarchitectural analysis (llvm-mca).",
                source_file.display()
            ));
        }

        let source_file_arg = if source_file.exists() {
            Some(source_file.as_path())
        } else {
            None
        };

        Self::profile(
            &binary,
            input_path.as_deref(),
            asm_file.as_deref(),
            source_file_arg,
            Some(Path::new(&config.build.compiler)),
        )
    }

    pub fn profile(
        binary: &Path,
        input: Option<&Path>,
        asm: Option<&Path>,
        source: Option<&Path>,
        compiler: Option<&Path>,
    ) -> Result<()> {
        let argo_dir = PathUtilities::parent_or_default(binary);
        let profiler_path = argo_dir.join("profiler.py");

        std::fs::write(&profiler_path, include_str!("profiler.py"))
            .map_err(|e| anyhow::anyhow!("Failed to write profiler script: {}", e))?;

        let mut cmd = Command::new("python3");
        cmd.arg(&profiler_path).arg(binary);

        if let Some(i) = input {
            cmd.arg(i);
            Ui::meta("input", i.display());
        } else {
            cmd.arg("none");
            cmd.stdin(Stdio::inherit());
            Ui::meta("input", "interactive");
        }

        if let Some(a) = asm {
            cmd.arg(a);
        } else {
            cmd.arg("none");
        }

        if let Some(s) = source {
            cmd.arg(s);
        } else {
            cmd.arg("none");
        }

        if let Some(c) = compiler {
            cmd.arg(c);
        } else {
            cmd.arg("none");
        }

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn Python profiler wrapper: {}", e))?;

        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                if let Some(section) = line.strip_prefix("@@ARGO_SECTION@@") {
                    Ui::section(section);
                } else if let Some(stat) = line.strip_prefix("@@ARGO_STAT@@") {
                    let parts: Vec<&str> = stat.splitn(2, "@@").collect();
                    if parts.len() == 2 {
                        Ui::meta(parts[0], parts[1]);
                    }
                } else if let Some(info) = line.strip_prefix("@@ARGO_INFO@@") {
                    println!("  {} {}", "↳".dimmed(), info.cyan());
                } else if let Some(err) = line.strip_prefix("@@ARGO_ERR@@") {
                    Ui::warn(err);
                }
            }
        }

        let status = child.wait()?;
        if !status.success() {
            let mut stderr_msg = String::new();
            if let Some(mut stderr) = child.stderr.take() {
                let mut reader = BufReader::new(&mut stderr);
                let _ = reader.read_line(&mut stderr_msg);
            }
            if !stderr_msg.trim().is_empty() {
                Ui::fail(format!("Profiler engine failed: {}", stderr_msg.trim()));
            }
        }

        Ok(())
    }
}
