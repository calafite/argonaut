use crate::utils::ui::Ui;
use anyhow::{Result, anyhow};
use colored::Colorize;
use inquire::Confirm;
use libc::{SIGABRT, SIGBUS, SIGFPE, SIGILL, SIGSEGV}; // <--- Native OS flags
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Instant;

#[cfg(target_os = "linux")]
use std::os::unix::process::CommandExt;

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

#[derive(Debug, Clone, Copy)]
pub struct RunnerFlags {
    pub bt_limit: usize,
    pub io_buf_size: usize,
}

impl Default for RunnerFlags {
    fn default() -> Self {
        Self {
            bt_limit: 15,
            io_buf_size: 1024,
        }
    }
}

pub struct Runner;

impl Runner {
    pub fn resolve_input(force_input: bool, force_no_input: bool) -> Result<bool> {
        let input_file = Path::new("input.txt");

        if force_input {
            Ok(true)
        } else if force_no_input {
            Ok(false)
        } else if input_file.exists() {
            let choice = Confirm::new("Found input.txt. Use it for stdin?")
                .with_default(true)
                .prompt()?;
            Ok(choice)
        } else {
            Ok(false)
        }
    }

    /// Backward-compatible facade calling default flags
    pub fn run(binary: &Path, use_file: bool) -> Result<()> {
        Self::run_with_flags(binary, use_file, RunnerFlags::default())
    }

    pub fn run_with_flags(binary: &Path, use_file: bool, flags: RunnerFlags) -> Result<()> {
        let input_file = Path::new("input.txt");
        let mut child_cmd = Command::new(binary);

        #[cfg(target_os = "linux")]
        {
            unsafe {
                child_cmd.pre_exec(|| {
                    crate::core::sandbox::apply_sandbox()
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::PermissionDenied, e))
                });
            }
        }

        if use_file {
            if !input_file.exists() {
                return Err(anyhow!("input.txt not found"));
            }
            Ui::meta("input", "input.txt");
            let file = File::open(input_file)?;
            child_cmd.stdin(Stdio::from(file));
        } else {
            Ui::meta("input", "interactive (your typing is default color)");
            child_cmd.stdin(Stdio::inherit());
        }

        child_cmd.stdout(Stdio::piped());
        child_cmd.stderr(Stdio::piped());

        println!();

        let start = Instant::now();
        let mut child = child_cmd.spawn()?;

        let mut child_stdout = child.stdout.take().expect("Failed to open stdout");
        let mut child_stderr = child.stderr.take().expect("Failed to open stderr");

        let buf_sz = flags.io_buf_size;

        let stdout_thread = thread::spawn(move || {
            let mut buf = vec![0u8; buf_sz];
            let mut out = io::stdout().lock();
            while let Ok(n) = child_stdout.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let _ = out.write_all(b"\x1b[1;96m");
                let _ = out.write_all(&buf[..n]);
                let _ = out.write_all(b"\x1b[0m");
                let _ = out.flush();
            }
        });

        let stderr_thread = thread::spawn(move || {
            let mut buf = vec![0u8; buf_sz];
            let mut err = io::stderr().lock();
            while let Ok(n) = child_stderr.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let _ = err.write_all(b"\x1b[1;91m");
                let _ = err.write_all(&buf[..n]);
                let _ = err.write_all(b"\x1b[0m");
                let _ = err.flush();
            }
        });

        let status = child.wait()?;
        let duration = start.elapsed();

        let _ = stdout_thread.join();
        let _ = stderr_thread.join();

        println!();

        if !status.success() {
            #[cfg(unix)]
            {
                if let Some(sig) = status.signal() {
                    let sig_desc = match sig {
                        SIGILL => "SIGILL (Illegal Instruction)",
                        SIGABRT => "SIGABRT (Aborted / Failed Assertion)",
                        SIGBUS => "SIGBUS (Bus Error / Misaligned Address)",
                        SIGFPE => "SIGFPE (Division by Zero / Float Trap)",
                        SIGSEGV => "SIGSEGV (Segmentation Fault)",
                        _ => "",
                    };

                    if !sig_desc.is_empty() {
                        Ui::fail(format!("process terminated by {sig_desc}"));
                        print_gdb_trace(binary, use_file, flags.bt_limit);
                        Ui::time(duration);
                        return Ok(());
                    }
                }
            }

            Ui::fail(format!("process exited with {}", status));
        }

        Ui::time(duration);
        Ok(())
    }
}

fn print_gdb_trace(binary: &Path, use_file: bool, bt_limit: usize) {
    let run_redirect = if use_file {
        "run < input.txt"
    } else {
        "run < /dev/null"
    };
    let limit_cmd = format!("set backtrace limit {bt_limit}");

    let mut gdb = Command::new("gdb");
    gdb.args([
        "-q",
        "-batch",
        "-ex",
        &limit_cmd,
        "-ex",
        run_redirect,
        "-ex",
        "bt",
        binary.to_str().unwrap_or(""),
    ]);

    if let Ok(out) = gdb.output() {
        let combined = format!(
            "{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );

        let mut missing_symbols = false;

        Ui::section("Instant GDB Stack Trace");

        for line in combined.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty()
                || trimmed.starts_with("[Thread")
                || trimmed.starts_with("[New")
                || trimmed.starts_with("[Detaching")
                || trimmed.starts_with("Using host")
                || trimmed.starts_with("Inferior 1")
            {
                continue;
            }

            if trimmed.contains("?? ()") {
                missing_symbols = true;
            }

            if trimmed.starts_with('#') {
                println!("  {}", line.cyan().bold());
            } else if trimmed.starts_with("Program received") {
                println!("  {}", line.red().bold());
            } else if trimmed.chars().next().map_or(false, |c| c.is_ascii_digit())
                && trimmed.contains('\t')
            {
                println!("  {}", line.yellow().bold());
            } else {
                println!("  {}", line);
            }
        }

        if missing_symbols {
            println!();
            Ui::info(
                "Trace contains '?? ()'. Re-compile via `argo debug` to see exact C++ line numbers.",
            );
        }
    } else {
        Ui::warn(
            "Could not execute 'gdb' — install GDB in your PATH to get automated crash traces.",
        );
    }
}
