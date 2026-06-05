use crate::ui::Ui;
use anyhow::{Result, anyhow};
use inquire::Confirm;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Instant;

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
        let input_file = Path::new("input.txt");
        let mut child_cmd = Command::new(binary);

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

        let stdout_thread = thread::spawn(move || {
            let mut buf = [0; 1024];
            let mut out = io::stdout();
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
            let mut buf = [0; 1024];
            let mut err = io::stderr();
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
            Ui::fail(format!("process exited with {}", status));
        }

        Ui::time(duration);
        Ok(())
    }
}
