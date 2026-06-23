use crate::config::settings::Config;
use crate::utils::{paths::expand_path, ui::Ui};
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub struct Scaffold;

impl Scaffold {
    pub fn create(dir: &Path, name: &str, config: &Config) -> Result<()> {
        if !dir.exists() {
            fs::create_dir_all(dir)
                .with_context(|| format!("Failed to create directory: {}", dir.display()))?;
        }

        let mut target_file = dir.join(name);
        target_file.set_extension("cpp");

        if let Some(template_str) = &config.scaffold.template_path {
            let template_path = expand_path(template_str);
            if template_path.exists() {
                fs::copy(&template_path, &target_file).with_context(|| {
                    format!("Failed to copy template from {}", template_path.display())
                })?;
                Ui::ok(format!("created {} (from template)", target_file.display()));
                return Ok(());
            } else {
                Ui::warn(format!(
                    "template not found at: {}",
                    template_path.display()
                ));
            }
        }

        Ui::warn("creating empty file instead");
        fs::write(&target_file, "")
            .with_context(|| format!("Failed to write empty file: {}", target_file.display()))?;

        Ui::ok(format!("created {}", target_file.display()));
        Ok(())
    }
}
