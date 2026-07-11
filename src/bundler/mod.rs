pub mod comment;
pub mod minify;
pub mod resolver;
pub mod strategies;

use crate::utils::paths::PathUtilities;
use crate::utils::ui::Ui;
use anyhow::Result;
use resolver::Resolver;
use std::path::{Path, PathBuf};
use strategies::{BundleStrategy, tree_sitter::TreeSitterShaker};

pub const LINE_BREAK: char = '\n';
pub const CARRIAGE_RETURN: char = '\r';
pub const BACKSLASH: char = '\\';
pub const UNDERSCORE: char = '_';
pub const DOUBLE_UNDERSCORE: &str = "__";
pub const SLASH: char = '/';
pub const STAR: char = '*';
pub const HASH: char = '#';
pub const QUOTE: char = '"';
pub const SINGLE_QUOTE: char = '\'';

pub struct Bundler {
    resolver: Resolver,
}

impl Bundler {
    pub fn execute_bundle(
        file: &Path,
        out: Option<&Path>,
        cli_includes: &[String],
        minify: bool,
        config: &crate::config::settings::Configuration,
    ) -> Result<()> {
        let directories = PathUtilities::get_include_dirs(cli_includes, config, file);

        Ui::section("Bundler");
        Ui::meta("source", file.display());

        let bundler = Self::new(directories);
        let mut bundled = bundler.bundle(file)?;

        if minify {
            Ui::meta("minify", "enabled");
            let original_len = bundled.len();
            bundled = crate::bundler::minify::Minifier::minify(&bundled);
            let new_len = bundled.len();
            Ui::info(format!(
                "Compressed from {} bytes to {} bytes ({:.1}% reduction)",
                original_len,
                new_len,
                100.0 - (new_len as f64 / original_len as f64) * 100.0
            ));
        }

        let out_path = match out {
            Some(path) => path.to_path_buf(),
            None => {
                let stem = file.file_stem().unwrap_or_default().to_string_lossy();
                file.with_file_name(format!("{}_bundled.cpp", stem))
            }
        };

        std::fs::write(&out_path, bundled)?;
        Ui::ok(format!("bundled to {}", out_path.display()));
        Ok(())
    }

    pub fn new(include_dirs: Vec<PathBuf>) -> Self {
        Self {
            resolver: Resolver::new(include_dirs),
        }
    }

    pub fn bundle(&self, entry_point: &Path) -> Result<String> {
        if !entry_point.is_file() {
            anyhow::bail!(
                "Invalid target: '{}' is a directory or does not exist.",
                entry_point.display()
            );
        }
        let mut strategy = TreeSitterShaker::new();
        strategy.bundle(entry_point, &self.resolver)
    }
}
