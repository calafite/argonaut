use crate::config::settings::Configuration;
use anyhow::Result;
use directories::{BaseDirs, ProjectDirs};
use std::path::{Path, PathBuf};

pub struct ProjectLayout {
    pub root: PathBuf,
}

impl ProjectLayout {
    const CACHE_DIR: &'static str = ".argo";

    pub fn new<P: Into<PathBuf>>(root: P) -> Self {
        Self { root: root.into() }
    }

    pub fn from_source_file(file: &Path) -> Self {
        let root = file.parent().unwrap_or_else(|| Path::new("."));
        Self::new(root)
    }

    pub fn from_binary(binary: &Path) -> Self {
        let root = binary
            .parent()
            .and_then(|path| path.parent())
            .unwrap_or_else(|| Path::new("."));
        Self::new(root)
    }

    pub fn root_dir(&self) -> &Path {
        &self.root
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.root.join(Self::CACHE_DIR)
    }

    pub fn source_file(&self, stem: &str) -> PathBuf {
        self.root.join(format!("{}.cpp", stem))
    }

    pub fn binary(&self, stem: &str) -> PathBuf {
        self.cache_dir().join(format!("{}.out", stem))
    }

    pub fn assembly_local(&self, stem: &str) -> PathBuf {
        self.root.join(format!("{}.s", stem))
    }

    pub fn assembly_cached(&self, stem: &str) -> PathBuf {
        self.cache_dir().join(format!("{}.s", stem))
    }

    pub fn tests_dir(&self, stem: &str) -> PathBuf {
        self.cache_dir().join("tests").join(stem)
    }

    pub fn default_input(&self) -> PathBuf {
        self.root.join("input.txt")
    }

    pub fn error_log(&self, stem: &str) -> PathBuf {
        self.cache_dir().join(format!("{}.errors", stem))
    }

    pub fn tracer_script(&self) -> PathBuf {
        self.cache_dir().join("tracer.py")
    }

    pub fn profiler_script(&self) -> PathBuf {
        self.cache_dir().join("profiler.py")
    }
}

pub struct PathUtilities;

impl PathUtilities {
    const QUALIFIER: &str = "";
    const ORGANIZATION: &str = "";
    const APPLICATION: &str = "argo";

    pub fn expand_path(path: &str) -> PathBuf {
        if (path.starts_with("~/") || path.starts_with("~\\"))
            && let Some(base_dirs) = BaseDirs::new()
        {
            return base_dirs.home_dir().join(&path[2..]);
        }
        PathBuf::from(path)
    }

    pub fn get_include_dirs(
        cli_includes: &[String],
        config: &Configuration,
        file: &Path,
    ) -> Vec<PathBuf> {
        let mut dirs: Vec<_> = config
            .build
            .include_dirs
            .iter()
            .map(|p| Self::expand_path(p))
            .collect();

        for inc in cli_includes {
            dirs.push(Self::expand_path(inc));
        }

        if let Ok(cpath) = std::env::var("CPATH") {
            for path in std::env::split_paths(&cpath) {
                dirs.push(Self::expand_path(&path.to_string_lossy()));
            }
        }

        if let Ok(cplus) = std::env::var("CPLUS_INCLUDE_PATH") {
            for path in std::env::split_paths(&cplus) {
                dirs.push(Self::expand_path(&path.to_string_lossy()));
            }
        }

        if let Ok(include_env) = std::env::var("INCLUDE") {
            for path in std::env::split_paths(&include_env) {
                dirs.push(Self::expand_path(&path.to_string_lossy()));
            }
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

    pub fn create_config(path: &PathBuf) -> Result<()> {
        match std::fs::create_dir_all(path) {
            Ok(_) => Ok(()),
            Err(_) => Err(anyhow::anyhow!(
                "Failed to create central configuration directory."
            )),
        }
    }

    pub fn project_dirs() -> Option<ProjectDirs> {
        ProjectDirs::from(Self::QUALIFIER, Self::ORGANIZATION, Self::APPLICATION)
    }

    pub fn parent_or_default(file: &Path) -> &Path {
        match file.parent() {
            Some(parent) => parent,
            None => Path::new("."),
        }
    }
}
