use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::Deserialize;
use std::fs;

#[derive(Deserialize, Default, Debug)]
pub struct Config {
    #[serde(default)]
    pub scaffold: ScaffoldConfig,
    #[serde(default)]
    pub build: BuildConfig,
}

#[derive(Deserialize, Default, Debug)]
pub struct ScaffoldConfig {
    pub template_path: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct BuildConfig {
    #[serde(default)]
    pub include_dirs: Vec<String>,
    #[serde(default = "default_compiler")]
    pub compiler: String,
    #[serde(default)]
    pub log_file: bool,
}

fn default_compiler() -> String {
    "g++".to_string()
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            include_dirs: Vec::new(),
            compiler: default_compiler(),
            log_file: false,
        }
    }
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
}
