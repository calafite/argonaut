use crate::ui::Ui;
use anyhow::{Result, anyhow};
use inquire::Confirm;
use std::fs::File;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Instant;

pub struct Runner;

impl Runner {
    pub fn run(binary: &Path, force_input: bool, force_no_input: bool) -> Result<()> {
        let mut use_file = false;
        let input_file = Path::new("input.txt");

        if force_input {
            use_file = true;
        } else if force_no_input {
            use_file = false;
        } else if input_file.exists() {
            use_file = Confirm::new("Found input.txt. Use it for stdin?")
                .with_default(true)
                .prompt()?;
        }

        let mut child_cmd = Command::new(binary);

        if use_file {
            if !input_file.exists() {
                return Err(anyhow!("input.txt not found"));
            }
            Ui::meta("input", "input.txt");
            let file = File::open(input_file)?;
            child_cmd.stdin(Stdio::from(file));
        } else {
            Ui::meta("input", "interactive");
            child_cmd.stdin(Stdio::inherit());
        }

        println!();

        let start = Instant::now();
        let mut child = child_cmd.spawn()?;
        let status = child.wait()?;
        let duration = start.elapsed();

        if !status.success() {
            Ui::fail(format!("process exited with {}", status));
        }

        Ui::time(duration);
        Ok(())
    }
}
