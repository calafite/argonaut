use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub struct Bundler {
    include_dirs: Vec<PathBuf>,
    visited: HashSet<PathBuf>,
    system_includes: HashSet<String>,
}

impl Bundler {
    pub fn new(include_dirs: Vec<PathBuf>) -> Self {
        Self {
            include_dirs,
            visited: HashSet::new(),
            system_includes: HashSet::new(),
        }
    }

    pub fn bundle(&mut self, file: &Path) -> Result<String> {
        let abs_path = file
            .canonicalize()
            .with_context(|| format!("Could not find file: {}", file.display()))?;
        self.process_file(&abs_path)
    }

    fn process_file(&mut self, file: &Path) -> Result<String> {
        if !self.visited.insert(file.to_path_buf()) {
            return Ok(String::new());
        }

        let content = fs::read_to_string(file)
            .with_context(|| format!("Failed to read {}", file.display()))?;

        let mut out = String::new();
        let mut pragma_once = false;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("#pragma once") {
                pragma_once = true;
                continue;
            }

            if let Some(inc) = parse_include(trimmed) {
                if let Some(resolved) = self.resolve_include(&inc, file) {
                    let inlined = self.process_file(&resolved)?;
                    if !inlined.is_empty() {
                        out.push_str(&inlined);
                        if !inlined.ends_with('\n') {
                            out.push('\n');
                        }
                    }
                    continue;
                } else {
                    let inc_str = if inc.is_quote {
                        format!("\"{}\"", inc.path)
                    } else {
                        format!("<{}>", inc.path)
                    };
                    if self.system_includes.insert(inc_str) {
                        out.push_str(line);
                        out.push('\n');
                    }
                    continue;
                }
            }

            out.push_str(line);
            out.push('\n');
        }

        if !pragma_once {
            self.visited.remove(file);
        }

        Ok(out)
    }

    fn resolve_include(&self, inc: &Include, current_file: &Path) -> Option<PathBuf> {
        let current_dir = current_file.parent().unwrap();

        if inc.is_quote {
            let candidate = current_dir.join(&inc.path);
            if candidate.exists() {
                return candidate.canonicalize().ok();
            }
        }

        for dir in &self.include_dirs {
            let candidate = dir.join(&inc.path);
            if candidate.exists() {
                return candidate.canonicalize().ok();
            }
        }

        None
    }
}

struct Include {
    path: String,
    is_quote: bool,
}

fn parse_include(line: &str) -> Option<Include> {
    let s = line.trim();
    if !s.starts_with('#') {
        return None;
    }
    let s = s[1..].trim();
    if !s.starts_with("include") {
        return None;
    }
    let s = s[7..].trim();

    if s.starts_with('"') {
        if let Some(end) = s[1..].find('"') {
            return Some(Include {
                path: s[1..=end].to_string(),
                is_quote: true,
            });
        }
    } else if s.starts_with('<') {
        if let Some(end) = s[1..].find('>') {
            return Some(Include {
                path: s[1..=end].to_string(),
                is_quote: false,
            });
        }
    }
    None
}
