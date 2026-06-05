use anyhow::{Context, Result};
use directories::{BaseDirs, ProjectDirs};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Deserialize, Default, Debug)]
pub struct Config {
    pub scaffold: ScaffoldConfig,
}

#[derive(Deserialize, Default, Debug)]
pub struct ScaffoldConfig {
    pub template_path: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let proj_dirs = ProjectDirs::from("", "", "argo")
            .context("Could not determine user configuration directory")?;

        let config_dir = proj_dirs.config_dir();
        let config_file = config_dir.join("Config.toml");

        if !config_file.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(&config_file)
            .with_context(|| format!("Failed to read config file at {}", config_file.display()))?;

        let config: Config = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse config file at {}", config_file.display()))?;

        Ok(config)
    }

    pub fn expand_path(path: &str) -> PathBuf {
        if path.starts_with("~/") || path.starts_with("~\\") {
            if let Some(base_dirs) = BaseDirs::new() {
                return base_dirs.home_dir().join(&path[2..]);
            }
        }
        PathBuf::from(path)
    }
}
