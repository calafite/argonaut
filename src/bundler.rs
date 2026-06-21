use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
struct ParsedFile {
    content: String,
    local_includes: Vec<PathBuf>,
    system_includes: Vec<String>,
    provided_symbols: Vec<String>,
    all_tokens: HashSet<String>,
}

pub struct Bundler {
    include_dirs: Vec<PathBuf>,
    parsed_files: HashMap<PathBuf, ParsedFile>,
    active_files: HashSet<PathBuf>,
    system_includes: HashSet<String>,
}

impl Bundler {
    pub fn new(include_dirs: Vec<PathBuf>) -> Self {
        Self {
            include_dirs,
            parsed_files: HashMap::new(),
            active_files: HashSet::new(),
            system_includes: HashSet::new(),
        }
    }

    pub fn bundle(&mut self, file: &Path) -> Result<String> {
        let abs_path = file
            .canonicalize()
            .with_context(|| format!("Could not find file: {}", file.display()))?;

        // Phase 1: Parse all reachable files
        self.load_graph(&abs_path)?;

        // Phase 2: Resolve which files are actually used (Tree-Shaking)
        self.tree_shake(&abs_path);

        // Phase 3: Assemble the final bundled file
        Ok(self.assemble(&abs_path))
    }

    fn load_graph(&mut self, root: &Path) -> Result<()> {
        let mut queue = vec![root.to_path_buf()];
        let mut visited = HashSet::new();
        visited.insert(root.to_path_buf());

        while let Some(path) = queue.pop() {
            let content = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read {}", path.display()))?;

            let mut local_includes = Vec::new();
            let mut sys_includes = Vec::new();

            for line in content.lines() {
                if let Some(inc) = parse_include(line) {
                    if let Some(resolved) = self.resolve_include(&inc, &path) {
                        local_includes.push(resolved.clone());
                        if visited.insert(resolved.clone()) {
                            queue.push(resolved);
                        }
                    } else {
                        if inc.is_quote {
                            eprintln!(
                                "  ⚠ bundler: could not resolve local include {:?} from {:?} — \
                                 pass the library path with -I or add it to [build].include_dirs in Config.toml",
                                inc.path,
                                path.display()
                            );
                        }
                        let inc_str = if inc.is_quote {
                            format!("\"{}\"", inc.path)
                        } else {
                            format!("<{}>", inc.path)
                        };
                        sys_includes.push(inc_str);
                    }
                }
            }

            let provided_symbols = extract_symbols(&content);
            let all_tokens = extract_all_tokens(&content);

            self.parsed_files.insert(
                path.clone(),
                ParsedFile {
                    content,
                    local_includes,
                    system_includes: sys_includes,
                    provided_symbols,
                    all_tokens,
                },
            );
        }
        Ok(())
    }

    fn tree_shake(&mut self, root: &Path) {
        self.active_files.insert(root.to_path_buf());

        loop {
            let mut changed = false;
            let current_active: Vec<PathBuf> = self.active_files.iter().cloned().collect();

            for (path, file) in &self.parsed_files {
                if self.active_files.contains(path) {
                    continue; 
                }

                // Check if it's `#include`d by ANY active file
                let included_by_active = current_active
                    .iter()
                    .any(|act| self.parsed_files[act].local_includes.contains(path));

                if !included_by_active {
                    continue;
                }

                // If the file acts just as an umbrella or provides utility macros
                if file.provided_symbols.is_empty() {
                    self.active_files.insert(path.clone());
                    changed = true;
                } else {
                    // It defines structures/classes. Check if ANY active file uses ANY of them.
                    let is_used = file.provided_symbols.iter().any(|sym| {
                        current_active
                            .iter()
                            .any(|act| self.parsed_files[act].all_tokens.contains(sym))
                    });

                    if is_used {
                        self.active_files.insert(path.clone());
                        changed = true;
                    }
                }
            }

            if !changed {
                break;
            }
        }
    }

    fn assemble(&mut self, root: &Path) -> String {
        // Hoist all system includes from ACTIVE files to the very top
        let mut all_sys_includes: Vec<String> = Vec::new();
        for path in &self.active_files {
            if let Some(parsed) = self.parsed_files.get(path) {
                for sys_inc in &parsed.system_includes {
                    if self.system_includes.insert(sys_inc.clone()) {
                        all_sys_includes.push(sys_inc.clone());
                    }
                }
            }
        }
        all_sys_includes.sort();

        let mut out = String::new();
        for sys_inc in all_sys_includes {
            out.push_str(&format!("#include {}\n", sys_inc));
        }
        if !self.system_includes.is_empty() {
            out.push('\n');
        }

        // Inline only the active local includes
        let mut emitted = HashSet::new();
        self.assemble_file(root, &mut emitted, &mut out);

        // Optional format cleanups
        out = out.replace("\n\n\n", "\n\n");
        out
    }

    fn assemble_file(&self, path: &Path, emitted: &mut HashSet<PathBuf>, out: &mut String) {
        if !emitted.insert(path.to_path_buf()) {
            return;
        }

        let parsed = &self.parsed_files[path];

        for line in parsed.content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("#pragma once") {
                continue; 
            }

            if let Some(inc) = parse_include(trimmed) {
                if let Some(resolved) = self.resolve_include(&inc, path) {
                    if self.active_files.contains(&resolved) {
                        self.assemble_file(&resolved, emitted, out);
                    } else {
                        // Tree-shaking excluded this file; preserve the include so the
                        // bundled file still compiles if the tree-shaker was wrong.
                        out.push_str(line);
                        out.push('\n');
                    }
                    continue;
                } else {
                    continue;
                }
            }

            out.push_str(line);
            out.push('\n');
        }
    }

    fn resolve_include(&self, inc: &Include, current_file: &Path) -> Option<PathBuf> {
        let current_dir = current_file.parent().unwrap();

        if inc.is_quote {
            let candidate = current_dir.join(&inc.path);
            if candidate.exists() {
                return candidate.canonicalize().ok().or_else(|| Some(candidate));
            }
        }

        for dir in &self.include_dirs {
            let candidate = dir.join(&inc.path);
            if candidate.exists() {
                return candidate.canonicalize().ok().or_else(|| Some(candidate));
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
    if !s.starts_with('#') { return None; }
    let s = s[1..].trim();
    if !s.starts_with("include") { return None; }
    let s = s[7..].trim();

    if s.starts_with('"') {
        if let Some(end) = s[1..].find('"') {
            return Some(Include { path: s[1..=end].to_string(), is_quote: true });
        }
    } else if s.starts_with('<') {
        if let Some(end) = s[1..].find('>') {
            return Some(Include { path: s[1..=end].to_string(), is_quote: false });
        }
    }
    None
}

fn extract_symbols(content: &str) -> Vec<String> {
    let mut symbols = Vec::new();
    
    let tokens: Vec<&str> = content
        .lines()
        .filter(|l| !l.trim().starts_with("#include"))
        .flat_map(|l| l.split(|c: char| !c.is_alphanumeric() && c != '_'))
        .filter(|s| !s.is_empty())
        .collect();

    for i in 0..tokens.len().saturating_sub(1) {
        if tokens[i] == "struct" || tokens[i] == "class" {
            let next = tokens[i + 1];
            if next != "public" && next != "private" {
                symbols.push(next.to_string());
            }
        }
    }
    symbols
}

fn extract_all_tokens(content: &str) -> HashSet<String> {
    content
        .lines()
        .filter(|l| !l.trim().starts_with("#include"))
        .flat_map(|l| l.split(|c: char| !c.is_alphanumeric() && c != '_'))
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}
