use crate::config::settings::Configuration;
use crate::utils::{paths::PathUtilities, ui::Ui};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
pub struct Scaffold;

impl Scaffold {
    pub fn create(name: &str, config: &Configuration) -> Result<()> {
        let current_directory = Self::current_directory()?;
        let directory = current_directory.join(name);

        Self::ensure_directory(&directory)?;

        let target_file = directory.join(name).with_extension("cpp");

        let template_path = Self::get_template(config);

        if let Some(path) = template_path {
            if path.exists() {
                Self::from_template(&path, &target_file)?;
                return Ok(());
            } else {
                Ui::warn(format!("template not found at: {}", path.display()));
            }
        }

        Self::create_empty(&target_file)?;
        Ok(())
    }

    fn ensure_directory(directory: &Path) -> Result<()> {
        if !directory.exists() {
            let closure = || format!("Failed to create directory: {}", directory.display());
            std::fs::create_dir_all(directory).with_context(closure)?;
        }
        Ok(())
    }

    fn current_directory() -> Result<PathBuf> {
        std::env::current_dir().context("Failed to determine current directory")
    }

    fn get_template(config: &Configuration) -> Option<PathBuf> {
        config
            .scaffold
            .template_path
            .as_ref()
            .map(|ts| PathUtilities::expand_path(ts))
    }

    fn create_empty(target_file: &Path) -> Result<()> {
        Ui::warn("creating empty file instead");
        let closure = || format!("Failed to write empty file: {}", target_file.display());
        fs::write(target_file, "").with_context(closure)?;
        Ui::ok(format!("created {}", target_file.display()));
        Ok(())
    }

    fn from_template(template: &Path, target: &Path) -> Result<()> {
        let closure = || format!("Failed to copy template from: {}", template.display());
        fs::copy(template, target).with_context(closure)?;
        Ui::ok(format!("created {} (from template)", target.display()));
        Ok(())
    }
}
