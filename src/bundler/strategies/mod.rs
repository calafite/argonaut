use crate::bundler::resolver::Resolver;
use anyhow::Result;
use std::path::Path;

pub mod topological;
pub mod tree_sitter;

pub trait BundleStrategy {
    fn bundle(&mut self, entry: &Path, resolver: &Resolver) -> Result<String>;
}
