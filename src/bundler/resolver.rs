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

    pub fn resolve(&self, include: &Include, current_file: &Path) -> Option<PathBuf> {
        let parent = current_file.parent();
        if include.is_quote && parent.is_some() {
            let parent = parent.unwrap();
            let local = parent.join(&include.path);
            if local.exists() {
                return Some(Self::canonicalize(local));
            }
        }

        for directory in &self.include_dirs {
            let candidate = directory.join(&include.path);
            if candidate.exists() {
                return Some(Self::canonicalize(candidate));
            }
        }
        None
    }

    fn canonicalize(path: PathBuf) -> PathBuf {
        path.canonicalize().unwrap_or(path)
    }
}

pub fn parse_include(line: &str) -> Option<Include> {
    let target = strip_include(line)?;
    extract_include(target)
}
fn strip_include(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let after_hash = trimmed.strip_prefix("#")?.trim_start();
    let target = after_hash.strip_prefix("include")?.trim();
    Some(target)
}

fn extract_include(target: &str) -> Option<Include> {
    let str_like = target.strip_prefix('"');
    if str_like.is_some() {
        let rest = str_like.unwrap();
        let end_index = rest.find('"')?;
        return Some(Include {
            path: rest[..end_index].to_string(),
            is_quote: true,
        });
    }

    let bracket_include = target.strip_prefix("<");
    if bracket_include.is_some() {
        let rest = bracket_include.unwrap();
        let end_index = rest.find('>')?;
        return Some(Include {
            path: rest[..end_index].to_string(),
            is_quote: false,
        });
    }

    None
}
