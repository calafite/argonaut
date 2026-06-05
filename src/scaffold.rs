use crate::ui::Ui;
use anyhow::{Result, anyhow};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub struct Scaffold;

impl Scaffold {
    pub fn create(dir: &Path, name: &str) -> Result<()> {
        if !dir.exists() {
            fs::create_dir_all(dir)?;
        }

        let mut target_file = dir.join(name);
        target_file.set_extension("cpp");

        let home = env::var("HOME").map_err(|_| anyhow!("HOME environment variable not set"))?;
        let template_path = PathBuf::from(home).join(".repos/data_structures/template.cpp");

        if template_path.exists() {
            fs::copy(&template_path, &target_file)?;
        } else {
            Ui::warn(format!("template not found: {}", template_path.display()));
            Ui::warn("creating empty file instead");
            fs::write(&target_file, "")?;
        }

        Ui::ok(target_file.display());
        Ok(())
    }
}
