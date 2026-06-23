use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Include {
    pub path: String,
    pub is_quote: bool,
}

pub struct Resolver {
    include_dirs: Vec<PathBuf>,
}

impl Resolver {
    pub fn new(include_dirs: Vec<PathBuf>) -> Self {
        Self { include_dirs }
    }

    pub fn resolve(&self, inc: &Include, current_file: &Path) -> Option<PathBuf> {
        if let Some(parent) = current_file.parent()
            && inc.is_quote
        {
            let local = parent.join(&inc.path);
            if local.exists() {
                return local.canonicalize().ok().or(Some(local));
            }
        }

        for dir in &self.include_dirs {
            let candidate = dir.join(&inc.path);
            if candidate.exists() {
                return candidate.canonicalize().ok().or(Some(candidate));
            }
        }
        None
    }
}

pub fn parse_include(line: &str) -> Option<Include> {
    let trimmed = line.trim();
    if !trimmed.starts_with('#') {
        return None;
    }
    let after_hash = trimmed[1..].trim_start();
    if !after_hash.starts_with("include") {
        return None;
    }
    let target = after_hash[7..].trim();

    if let (Some(rest), Some(end)) = (target.strip_prefix('"'), target.rfind('"'))
        && end > 0
    {
        Some(Include {
            path: rest[..end - 1].to_string(),
            is_quote: true,
        })
    } else if let (Some(rest), Some(end)) = (target.strip_prefix('<'), target.rfind('>'))
        && end > 0
    {
        Some(Include {
            path: rest[..end - 1].to_string(),
            is_quote: false,
        })
    } else {
        None
    }
}
