use crate::config::settings::Config;
use directories::BaseDirs;
use std::path::{Path, PathBuf};

pub fn expand_path(path: &str) -> PathBuf {
    if (path.starts_with("~/") || path.starts_with("~\\"))
        && let Some(base_dirs) = BaseDirs::new()
    {
        return base_dirs.home_dir().join(&path[2..]);
    }
    PathBuf::from(path)
}

pub fn get_include_dirs(cli_includes: &[String], config: &Config, file: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<_> = config
        .build
        .include_dirs
        .iter()
        .map(|p| expand_path(p))
        .collect();

    for inc in cli_includes {
        dirs.push(expand_path(inc));
    }

    let abs_file = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());

    if let Some(parent) = abs_file.parent() {
        let parent_include = parent.join("include");
        if parent_include.exists() {
            dirs.push(parent_include);
        }
        dirs.push(parent.to_path_buf());
    }

    if let Ok(cwd) = std::env::current_dir() {
        let cwd_include = cwd.join("include");
        if cwd_include.exists() {
            dirs.push(cwd_include);
        }
        dirs.push(cwd);
    }

    let mut resolved = Vec::new();
    for d in dirs {
        let canon = d.canonicalize().unwrap_or(d);
        if !resolved.contains(&canon) {
            resolved.push(canon);
        }
    }

    resolved
}
