use crate::core::compiler::Compiler;
use crate::core::runner::Runner;
use crate::utils::ui::Ui;
use anyhow::{Context, Result};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

pub struct Watcher;

impl Watcher {
    fn clear_screen() {
        print!("{esc}[2J{esc}[1;1H", esc = 27 as char);
    }

    pub fn watch(file: &Path, use_file: bool, include_dirs: &[PathBuf]) -> Result<()> {
        let file = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());
        let parent_dir = file.parent().context("Invalid file path")?;
        let file_name = file.file_name().context("Invalid file name")?;

        let (tx, rx) = channel();
        let mut watcher = RecommendedWatcher::new(tx, Config::default())?;

        watcher.watch(parent_dir, RecursiveMode::NonRecursive)?;

        Self::clear_screen();
        Ui::info(format!(
            "watching {} — rebuild on save (Ctrl-C to stop)",
            file.display()
        ));

        if let Ok(binary) = Compiler::build(&file, true, include_dirs) {
            let _ = Runner::run(&binary, use_file);
        }

        let mut last_compile = Instant::now();
        let debounce_duration = Duration::from_millis(300);

        loop {
            match rx.recv() {
                Ok(Ok(event)) => {
                    let involves_our_file =
                        event.paths.iter().any(|p| p.file_name() == Some(file_name));

                    if involves_our_file && last_compile.elapsed() > debounce_duration {
                        std::thread::sleep(Duration::from_millis(50));
                        // Flush any pending events during sleep
                        while rx.try_recv().is_ok() {}

                        last_compile = Instant::now();

                        Self::clear_screen();
                        Ui::info("file changed — recompiling...");

                        if let Ok(binary) = Compiler::build(&file, true, include_dirs) {
                            let _ = Runner::run(&binary, use_file);
                        }
                    }
                }
                Ok(Err(e)) => Ui::warn(format!("watch error: {:?}", e)),
                Err(_) => break,
            }
        }

        Ok(())
    }
}
