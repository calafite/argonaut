pub mod comment;
pub mod minify;
pub mod resolver;
pub mod strategies;

use anyhow::Result;
use resolver::Resolver;
use std::path::{Path, PathBuf};
use strategies::{BundleStrategy, tree_sitter::TreeSitterShaker};

pub struct Bundler {
    resolver: Resolver,
}

impl Bundler {
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
