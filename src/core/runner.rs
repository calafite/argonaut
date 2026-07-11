use crate::utils::paths::PathUtilities;
use crate::utils::ui::Ui;
use anyhow::Result;
use colored::Colorize;
use inquire::Confirm;
use libc::{RUSAGE_CHILDREN, SIGABRT, SIGBUS, SIGFPE, SIGILL, SIGSEGV, getrusage, rusage};
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Instant;

#[derive(Debug, Clone, Copy)]
pub struct RunnerFlags {
    pub bt_limit: usize,
    pub io_buf_size: usize,
    pub memory_limit_mb: Option<usize>,
}

impl Default for RunnerFlags {
    fn default() -> Self {
        Self {
            bt_limit: 30,
            io_buf_size: 8224,
            memory_limit_mb: None,
        }
    }
}

pub struct ExecutionResult {
    pub status: std::process::ExitStatus,
    pub stdout: String,
    pub stderr: String,
    pub time_nanos: u128,
}

struct PythonTraceFrame {
    func: String,
    file: String,
    line_num: String,
    code: String,
}

struct PythonTrace {
    reason: Option<String>,
    frames: Vec<PythonTraceFrame>,
    missing_symbols: bool,
}

struct FallbackTrace {
    frames: Vec<String>,
    crash_reason: String,
    offending_line: String,
    missing_symbols: bool,
}

pub struct Runner;

impl Runner {
    fn input_file(binary: &Path) -> PathBuf {
        let parent = PathUtilities::parent_or_default(binary);
        parent.join("input.txt")
    }

    pub fn resolve_input(binary: &Path, force_input: bool, no_input: bool) -> Result<bool> {
        if force_input {
            return Ok(true);
        }

        if no_input {
            return Ok(false);
        }

        let input_file = Self::input_file(binary);
        if input_file.exists() {
            let choice = Confirm::new(&format!(
                "Found {}. Use it for stdin?",
                input_file.display(),
            ))
            .with_default(true)
            .prompt()?;

            Ok(choice)
        } else {
            Ok(false)
        }
    }

    pub fn run(binary: &Path, use_file: bool) -> Result<()> {
        Self::with_flags(binary, use_file, RunnerFlags::default())
    }

    fn with_flags(binary: &Path, use_file: bool, flags: RunnerFlags) -> Result<()> {
        let input_file = Self::input_file(binary);
        let mut child_command = Self::child_command(binary, use_file, &input_file, flags)?;

        println!();

        #[cfg(unix)]
        let start_ns = Self::children_nanos();
        let start = Instant::now();
        let mut child = child_command.spawn()?;

        let child_stdout = child.stdout.take().expect("Failed to open stdout");
        let child_stderr = child.stderr.take().expect("Failed to open stderr");

        let stdout_thread =
            Self::stream_thread(child_stdout, io::stdout(), flags.io_buf_size, b"\x1b[1;93m");

        let stderr_thread =
            Self::stream_thread(child_stderr, io::stderr(), flags.io_buf_size, b"\x1b[1;91m");

        let status = child.wait()?;
        let wall_nanos = start.elapsed().as_nanos();

        #[cfg(unix)]
        let cpu_nanos = Self::children_nanos().saturating_sub(start_ns);
        let exec_time = if use_file { wall_nanos } else { cpu_nanos };

        if stdout_thread.join().is_err() {
            Ui::fail("stdout thread panicked on join.")
        };

        if stderr_thread.join().is_err() {
            Ui::fail("stderr thread panicked on join.");
        };

        println!();

        Self::handle_exit(binary, use_file, status, exec_time, flags)
    }

    pub fn execute_captured(
        binary: &Path,
        input_file: &Path,
        flags: RunnerFlags,
    ) -> Result<ExecutionResult> {
        let mut child_command = Self::child_command(binary, true, input_file, flags)?;

        #[cfg(unix)]
        let start_ns = Self::children_nanos();
        let start = Instant::now();

        let output = child_command.output()?;
        let wall_nanos = start.elapsed().as_nanos();

        #[cfg(unix)]
        let cpu_nanos = Self::children_nanos().saturating_sub(start_ns);

        #[cfg(unix)]
        let exec_time = if cpu_nanos > 0 { cpu_nanos } else { wall_nanos };

        Ok(ExecutionResult {
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            time_nanos: exec_time,
        })
    }

    fn child_command(
        binary: &Path,
        use_file: bool,
        input_file: &Path,
        flags: RunnerFlags,
    ) -> Result<Command> {
        let mut child_command = Command::new(binary);

        child_command.env("OMP_NUM_THREADS", "1");
        child_command.env("MKL_NUM_THREADS", "1");
        child_command.env("OPENBLAS_NUM_THREADS", "1");
        child_command.env("VECLIB_MAXIMUM_THREADS", "1");
        child_command.env("NUMEXPR_NUM_THREADS", "1");

        #[cfg(unix)]
        {
            unsafe {
                let memory_limit = flags.memory_limit_mb;
                let closure = move || {
                    #[cfg(target_os = "linux")]
                    {
                        if let Err(e) = crate::core::sandbox::apply_sandbox() {
                            let kind = std::io::ErrorKind::PermissionDenied;
                            return Err(std::io::Error::new(kind, e));
                        }
                    }
                    Self::apply_limits(memory_limit)?;
                    Ok(())
                };
                child_command.pre_exec(closure);
            }
        }

        if use_file {
            if !input_file.exists() {
                return Err(anyhow::anyhow!("{} not found", input_file.display()));
            }
            Ui::meta("input", input_file.display());
            let file = File::open(input_file)?;
            child_command.stdin(Stdio::from(file));
        } else {
            Ui::meta("input", "interactive");
            child_command.stdin(Stdio::inherit());
        }

        child_command.stdout(Stdio::piped());
        child_command.stderr(Stdio::piped());
        Ok(child_command)
    }

    #[cfg(unix)]
    fn apply_limits(memory_limit_mb: Option<usize>) -> Result<(), std::io::Error> {
        // limit memory allocation
        if let Some(limit_mb) = memory_limit_mb {
            let limit_bytes = (limit_mb as u64) * 1024 * 1024;
            let rlim = libc::rlimit {
                rlim_cur: limit_bytes,
                rlim_max: limit_bytes,
            };
            unsafe {
                if libc::setrlimit(libc::RLIMIT_AS, &rlim) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
        }

        // limits execution to one core
        #[cfg(target_os = "linux")]
        unsafe {
            let mut set: libc::cpu_set_t = std::mem::zeroed();
            libc::CPU_SET(0, &mut set);

            let ret = libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set);
            if ret != 0 {
                return Err(std::io::Error::last_os_error());
            }
        }

        Ok(())
    }

    fn handle_exit(
        binary: &Path,
        use_file: bool,
        status: std::process::ExitStatus,
        exec_time: u128,
        flags: RunnerFlags,
    ) -> Result<()> {
        Ui::time(exec_time);

        if status.success() {
            return Ok(());
        }

        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            if let Some(signal) = status.signal() {
                let signal_description = match signal {
                    SIGILL => "SIGILL (Illegal Instruction)",
                    SIGABRT => "SIGABRT (Aborted / Failed Assertion)",
                    SIGBUS => "SIGBUS (Bus Error / Misaligned Address)",
                    SIGFPE => "SIGFPE (Division by Zero / Float Trap)",
                    SIGSEGV => "SIGSEGV (Segmentation Fault)",
                    _ => "",
                };

                if !signal_description.is_empty() {
                    Self::print_trace(binary, use_file, flags.bt_limit);
                    return Err(anyhow::anyhow!(
                        "process terminated by {signal_description}"
                    ));
                }
            }
        }

        Err(anyhow::anyhow!("process exited with {}", status))
    }

    fn write_tracer(argo_directory: &Path) -> Result<PathBuf> {
        std::fs::create_dir_all(argo_directory)?;
        let tracer_path = argo_directory.join("tracer.py");
        std::fs::write(&tracer_path, include_str!("tracer.py"))?;
        Ok(tracer_path)
    }

    fn print_trace(binary: &Path, use_file: bool, bt_limit: usize) {
        let input_redirect = if use_file {
            Self::input_file(binary).to_string_lossy().to_string()
        } else {
            "/dev/null".to_string()
        };

        let argo_directory = PathUtilities::parent_or_default(binary);
        let tracer_path = match Self::write_tracer(argo_directory) {
            Ok(path) => path,
            Err(_) => {
                Ui::warn("Could not write Python tracer. Falling back to CLI parsing...");
                return;
            }
        };

        let mut gdb = Command::new("gdb");
        gdb.env("ARGO_INPUT_REDIRECT", &input_redirect)
            .env("ARGO_BT_LIMIT", bt_limit.to_string())
            .env("LC_ALL", "C")
            .args([
                "-q",
                "-batch",
                "-x",
                tracer_path.to_str().unwrap_or(""),
                binary.to_str().unwrap_or(""),
            ]);

        let out = match gdb.output() {
            Ok(output) => output,
            Err(_) => {
                Ui::warn("Could not execute 'gdb'; ensure GDB is installed in your PATH.");
                return;
            }
        };

        let combined = format!(
            "{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );

        if combined.contains("Python scripting is not supported") || !combined.contains("@@ARGO_") {
            Self::trace_fallback(binary, &input_redirect, bt_limit);
            return;
        }

        let trace = Self::parse_python_trace(&combined);
        Self::print_python_trace(trace);
    }

    fn parse_python_trace(output: &str) -> PythonTrace {
        let mut reason = None;
        let mut frames = Vec::new();
        let mut missing_symbols = false;

        for line in output.lines() {
            if let Some(r) = line.strip_prefix("@@ARGO_REASON@@") {
                reason = Some(r.trim().to_string());
            } else if let Some(frame_data) = line.strip_prefix("@@ARGO_FRAME@@") {
                let parts: Vec<&str> = frame_data.splitn(4, "@@").collect();
                if parts.len() == 4 {
                    let func = parts[0].to_string();
                    let file = parts[1].to_string();
                    let line_num = parts[2].to_string();
                    let code = parts[3].to_string();

                    if func == "??" || file == "??" {
                        missing_symbols = true;
                    }

                    frames.push(PythonTraceFrame {
                        func,
                        file,
                        line_num,
                        code,
                    });
                }
            }
        }

        PythonTrace {
            reason,
            frames,
            missing_symbols,
        }
    }

    fn print_python_trace(trace: PythonTrace) {
        Ui::section("Automated Crash Trace");

        if let Some(reason) = trace.reason.clone() {
            println!("  {} {}", "💥".red(), reason.red().bold());
        }

        for frame in trace.frames {
            let display_file = if frame.file.starts_with('/') {
                Path::new(&frame.file)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            } else {
                frame.file.clone()
            };

            if frame.file != "??" && frame.line_num != "0" {
                println!(
                    "  {} {} at {}:{}",
                    "↳".dimmed(),
                    frame.func.cyan().bold(),
                    display_file.yellow(),
                    frame.line_num.yellow().bold()
                );
                if !frame.code.is_empty() {
                    println!("      {} {}", ">".red(), frame.code.white());
                }
            } else {
                println!("  {} {}", "↳".dimmed(), frame.func.cyan().bold());
            }
        }

        if trace.missing_symbols {
            println!();
            Ui::info(
                "Trace missing symbols ('??'). Rebuild via `argo debug` for exact line numbers.",
            );
        } else if trace.reason.is_none() {
            Ui::warn("GDB failed to isolate the crash. Run manually for details.");
        }
    }

    fn trace_fallback(binary: &Path, input_redirect: &str, bt_limit: usize) {
        let run_redirect = format!("run < {} > /dev/null 2>&1", input_redirect);
        let limit_command = format!("set backtrace limit {bt_limit}");
        let mut gdb = Command::new("gdb");
        gdb.env("LC_ALL", "C");
        gdb.args([
            "-q",
            "-batch",
            "-ex",
            "set print address off",
            "-ex",
            &limit_command,
            "-ex",
            &run_redirect,
            "-ex",
            "bt",
            binary.to_str().unwrap_or_default(),
        ]);

        if let Ok(output) = gdb.output() {
            let combined = format!(
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );

            let trace = Self::parse_fallback(&combined);
            Self::print_fallback_trace(trace);
        }
    }

    fn parse_fallback(output: &str) -> FallbackTrace {
        let mut frames = Vec::new();
        let mut crash_reason = String::new();
        let mut offending_line = String::new();
        let mut missing_symbols = false;

        for line in output.lines() {
            let trimmed = line.trim();
            if trimmed.contains("?? ()") {
                missing_symbols = true;
            }

            if trimmed.starts_with('#')
                && trimmed[1..]
                    .chars()
                    .next()
                    .is_some_and(|character| character.is_ascii_digit())
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

            if frames.is_empty()
                && let Some((line_number, _)) =
                    trimmed.split_once(|character: char| character.is_whitespace())
                && !line_number.is_empty()
                && line_number
                    .chars()
                    .all(|character| character.is_ascii_digit())
            {
                offending_line = trimmed.to_string();
            }
        }

        FallbackTrace {
            frames,
            crash_reason,
            offending_line,
            missing_symbols,
        }
    }

    fn print_fallback_trace(trace: FallbackTrace) {
        Ui::section("Instant GDB Stack Trace (Fallback Mode)");

        if !trace.crash_reason.is_empty() {
            println!("  {}", trace.crash_reason.red().bold());
        }
        if !trace.offending_line.is_empty() {
            println!("  {}", trace.offending_line.yellow().bold());
        }
        for frame in trace.frames {
            println!("  {}", frame.cyan().bold());
        }

        if trace.missing_symbols {
            println!();
            Ui::info(
                "Trace missing symbols ('??'). Rebuild via `argo debug` for exact line numbers.",
            );
        }
    }

    fn stream_thread<R, W>(
        mut reader: R,
        mut writer: W,
        buf_size: usize,
        color_prefix: &'static [u8],
    ) -> thread::JoinHandle<()>
    where
        R: Read + Send + 'static,
        W: Write + Send + 'static,
    {
        thread::spawn(move || {
            let mut buffer = vec![0u8; buf_size];
            while let Ok(number) = reader.read(&mut buffer) {
                if number == 0 {
                    break;
                }
                let _ = writer.write_all(color_prefix);
                let _ = writer.write_all(&buffer[..number]);
                let _ = writer.write_all(b"\x1b[0m");
                let _ = writer.flush();
            }
        })
    }

    #[cfg(unix)]
    fn children_nanos() -> u128 {
        let mut usage = std::mem::MaybeUninit::<rusage>::uninit();
        unsafe {
            if getrusage(RUSAGE_CHILDREN, usage.as_mut_ptr()) == 0 {
                let usage = usage.assume_init();
                let utime = (usage.ru_utime.tv_sec as u128) * 1_000_000_000
                    + (usage.ru_utime.tv_usec as u128) * 1_000;
                let stime = (usage.ru_stime.tv_sec as u128) * 1_000_000_000
                    + (usage.ru_stime.tv_usec as u128) * 1_000;
                utime + stime
            } else {
                0
            }
        }
    }
}
