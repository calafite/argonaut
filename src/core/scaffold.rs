use crate::config::settings::Configuration;
use crate::parser::payload::ProblemPayload;
use crate::utils::{paths::PathUtilities, ui::Ui};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub struct Scaffold;

impl Scaffold {
    const EMPTY: &str = "";

    pub fn execute_new(name: &str, config: &Configuration) -> Result<()> {
        Ui::section("Project Scaffold");
        Self::create(name, config)
    }

    pub fn create(name: &str, config: &Configuration) -> Result<()> {
        let target = Self::prepare_target(name)?;
        let template = Self::get_template(config);
        Self::build_write(&target, Self::EMPTY, template)
    }

    pub fn from_parsed(name: &str, payload: &ProblemPayload, config: &Configuration) -> Result<()> {
        let target = Self::prepare_target(name)?;
        let template = Self::get_template(config);

        let header = format!(
            "/**\n * Problem: {}\n * Group: {}\n * URL: {}\n * Time Limit: {} ms\n * Memory Limit: {} MB\n */\n\n",
            payload.name, payload.group, payload.url, payload.time_limit, payload.memory_limit
        );

        Self::build_write(&target, &header, template)
    }

    fn prepare_target(name: &str) -> Result<PathBuf> {
        let current = std::env::current_dir().context("Failed to determine current directory")?;
        let directory = current.join(name);

        if !directory.exists() {
            let closure = || format!("Failed to create directory layout: {}", directory.display());
            fs::create_dir_all(&directory).with_context(closure)?;
        }

        let result = directory.join(name).with_extension("cpp");
        Ok(result)
    }

    fn build_write(target: &Path, header: &str, template: Option<PathBuf>) -> Result<()> {
        let mut content = header.to_string();
        let mut used_template = false;

        if let Some(ref path) = template {
            if path.exists() {
                let closure = || format!("Failed to read template from: {}", path.display());
                let data = fs::read_to_string(path).with_context(closure)?;
                content.push_str(&data);
                used_template = true;
            } else {
                Ui::warn(format!("template not found at: {}", path.display()));
            }
        }

        if !used_template {
            if template.is_none() {
                Ui::warn("no template configured, creating empty file.");
            } else {
                Ui::warn("creating empty file instead");
            }
        }

        let closure = || format!("Failed to write source file: {}", target.display());
        fs::write(target, content).with_context(closure)?;

        if used_template {
            Ui::ok(format!("created {} (from template)", target.display()));
        } else {
            Ui::ok(format!("created {}", target.display()));
        }

        Ok(())
    }

    fn get_template(config: &Configuration) -> Option<PathBuf> {
        config
            .scaffold
            .template_path
            .as_ref()
            .map(|ts| PathUtilities::expand_path(ts))
    }
}
