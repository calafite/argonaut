use crate::utils::paths::PathUtilities;
use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::Deserialize;
use std::{fs, path::PathBuf};

const CONFIG_FILE: &str = "Config.toml";

#[derive(Deserialize, Default, Debug, Clone)]
pub struct Configuration {
    #[serde(default)]
    pub scaffold: ScaffoldConfig,
    #[serde(default)]
    pub build: BuildConfig,
}

#[derive(Deserialize, Default, Debug, Clone)]
pub struct ScaffoldConfig {
    pub template_path: Option<String>,
    pub short_name: bool,
}

#[derive(Deserialize, Debug, Clone)]
pub struct BuildConfig {
    #[serde(default)]
    pub include_dirs: Vec<String>,
    #[serde(default = "default_compiler")]
    pub compiler: String,
    #[serde(default = "default_std")]
    pub std: u32,
    #[serde(default)]
    pub log_file: bool,
}

fn default_compiler() -> String {
    String::from("g++")
}

fn default_std() -> u32 {
    20u32
}

fn config_path(project_dirs: ProjectDirs) -> PathBuf {
    project_dirs.config_dir().join(CONFIG_FILE)
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            include_dirs: Vec::new(),
            compiler: default_compiler(),
            std: default_std(),
            log_file: false,
        }
    }
}

impl Configuration {
    pub fn load() -> Result<Self> {
        let project_dirs = match PathUtilities::project_dirs() {
            Some(directories) => directories,
            None => {
                let error = String::from("Could not resolve configuration directory");
                return Err(anyhow::anyhow!(error));
            }
        };

        let config_file = config_path(project_dirs);

        if !config_file.exists() {
            return Self::ok_default();
        }

        let contents = match fs::read_to_string(&config_file) {
            Ok(contents) => contents,
            Err(error) => {
                let error_string = format!(
                    "Failed to read configuration file {}",
                    config_file.display()
                );
                return Err(error).context(error_string);
            }
        };

        let configuration: Configuration = match toml::from_str(&contents) {
            Ok(configuration) => configuration,
            Err(error) => {
                let error_string = format!(
                    "Failed to parse configuration file {}",
                    config_file.display()
                );
                return Err(error).context(error_string);
            }
        };

        Ok(configuration)
    }

    fn ok_default() -> Result<Self> {
        Ok(Self::default())
    }
}
