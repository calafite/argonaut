use super::BundleStrategy;
use crate::bundler::comment::strip_comments;
use crate::bundler::resolver::{Resolver, parse_include};
use anyhow::{Context, Result, bail};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub struct TopologicalInliner {
    emitted: HashSet<PathBuf>,
    call_stack: HashSet<PathBuf>,
    system_includes: HashSet<String>,
}

impl TopologicalInliner {
    pub fn new() -> Self {
        Self {
            emitted: HashSet::new(),
            call_stack: HashSet::new(),
            system_includes: HashSet::new(),
        }
    }

    fn dfs(&mut self, file: &Path, resolver: &Resolver, out: &mut String) -> Result<()> {
        let canon = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());

        if self.emitted.contains(&canon) {
            return Ok(());
        }
        if !self.call_stack.insert(canon.clone()) {
            bail!(
                "Circular #include dependency detected involving: {}",
                canon.display()
            );
        }

        let raw_content = fs::read_to_string(&canon)
            .with_context(|| format!("Failed to read source: {}", canon.display()))?;

        let cleaned_content = strip_comments(&raw_content);

        for (raw_line, clean_line) in raw_content.lines().zip(cleaned_content.lines()) {
            let trimmed_clean = clean_line.trim();

            if trimmed_clean.starts_with("#pragma once") {
                continue;
            }

            if let Some(inc) = parse_include(trimmed_clean) {
                if let Some(resolved_path) = resolver.resolve(&inc, &canon) {
                    self.dfs(&resolved_path, resolver, out)?;
                    continue;
                } else {
                    let formatted = if inc.is_quote {
                        format!("\"{}\"", inc.path)
                    } else {
                        format!("<{}>", inc.path)
                    };
                    self.system_includes.insert(formatted);
                    continue;
                }
            }

            out.push_str(raw_line);
            out.push('\n');
        }

        self.call_stack.remove(&canon);
        self.emitted.insert(canon);
        Ok(())
    }
}

impl BundleStrategy for TopologicalInliner {
    fn bundle(&mut self, entry: &Path, resolver: &Resolver) -> Result<String> {
        let mut body = String::new();
        self.dfs(entry, resolver, &mut body)?;

        let mut sys_list: Vec<_> = self.system_includes.iter().cloned().collect();
        sys_list.sort();

        let mut final_bundle = String::new();
        final_bundle
            .push_str("// ====================================================================\n");
        final_bundle.push_str("// Bundled by Argonaut\n");
        final_bundle.push_str(
            "// ====================================================================\n\n",
        );

        for sys in sys_list {
            final_bundle.push_str(&format!("#include {}\n", sys));
        }
        final_bundle.push_str("\n");
        final_bundle.push_str(&body);

        Ok(final_bundle
            .replace("\n\n\n\n", "\n\n")
            .replace("\n\n\n", "\n\n"))
    }
}
