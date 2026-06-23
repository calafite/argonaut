use crate::utils::ui::Ui;
use anyhow::{Result, anyhow};
use colored::Colorize;
use inquire::Confirm;
use libc::{RUSAGE_CHILDREN, SIGABRT, SIGBUS, SIGFPE, SIGILL, SIGSEGV, getrusage, rusage};
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

        #[cfg(unix)]
        let cpu_start_ns = get_children_cpu_nanos();

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
        let wall_nanos = start.elapsed().as_nanos();

        #[cfg(unix)]
        let cpu_nanos = get_children_cpu_nanos().saturating_sub(cpu_start_ns);
        let exec_time_ns = if use_file { wall_nanos } else { cpu_nanos };

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
                        Ui::time(exec_time_ns);
                        return Ok(());
                    }
                }
            }

            Ui::fail(format!("process exited with {}", status));
        }

        Ui::time(exec_time_ns);
        Ok(())
    }
}

#[cfg(unix)]
fn get_children_cpu_nanos() -> u128 {
    let mut usage = std::mem::MaybeUninit::<rusage>::uninit();
    unsafe {
        if getrusage(RUSAGE_CHILDREN, usage.as_mut_ptr()) == 0 {
            let u = usage.assume_init();
            let utime =
                (u.ru_utime.tv_sec as u128) * 1_000_000_000 + (u.ru_utime.tv_usec as u128) * 1_000;
            let stime =
                (u.ru_stime.tv_sec as u128) * 1_000_000_000 + (u.ru_stime.tv_usec as u128) * 1_000;
            utime + stime
        } else {
            0
        }
    }
}

fn print_gdb_trace(binary: &Path, use_file: bool, bt_limit: usize) {
    let run_redirect = if use_file {
        "run < input.txt > /dev/null 2>&1"
    } else {
        "run < /dev/null > /dev/null 2>&1"
    };

    let limit_cmd = format!("set backtrace limit {bt_limit}");

    let mut gdb = Command::new("gdb");

    gdb.env("LC_ALL", "C");
    gdb.args([
        "-q",
        "-batch",
        "-ex",
        "set print address off",
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

        let mut frames = Vec::new();
        let mut crash_reason = String::new();
        let mut offending_line = String::new();
        let mut missing_symbols = false;

        for line in combined.lines() {
            let trimmed = line.trim();

            if trimmed.contains("?? ()") {
                missing_symbols = true;
            }

            if trimmed.starts_with('#')
                && trimmed[1..]
                    .chars()
                    .next()
                    .map_or(false, |c| c.is_ascii_digit())
            {
                frames.push(trimmed.to_string());
                continue;
            }

            if trimmed.starts_with("Program received signal")
                || trimmed.starts_with("Program terminated")
            {
                crash_reason = trimmed.to_string();
                continue;
            }

            if frames.is_empty() {
                if let Some((line_num, _code)) = trimmed.split_once(|c: char| c.is_whitespace()) {
                    if !line_num.is_empty() && line_num.chars().all(|c| c.is_ascii_digit()) {
                        offending_line = trimmed.to_string();
                    }
                }
            }
        }

        Ui::section("Instant GDB Stack Trace");

        if !crash_reason.is_empty() {
            println!("  {}", crash_reason.red().bold());
        }

        if !offending_line.is_empty() {
            println!("  {}", offending_line.yellow().bold());
        }

        for frame in frames {
            println!("  {}", frame.cyan().bold());
        }

        if missing_symbols {
            println!();
            Ui::info(
                "Trace contains '?? ()'. Re-compile via `argo debug` to see exact C++ line numbers.",
            );
        } else if crash_reason.is_empty() {
            Ui::warn("Could not isolate crash context. Check GDB installation.");
        }
    } else {
        Ui::warn(
            "Could not execute 'gdb' — install GDB in your PATH to get automated crash traces.",
        );
    }
}
