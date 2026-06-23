use super::BundleStrategy;
use crate::bundler::comment::strip_comments;
use crate::bundler::resolver::{Resolver, parse_include};
use anyhow::{Context, Result};
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use tree_sitter::{Node, Parser};

#[derive(Debug, Clone)]
struct CodeBlock {
    raw_text: String,
    defined_symbols: Vec<String>,
    referenced_symbols: HashSet<String>,
    always_keep: bool,
}

pub struct TreeSitterShaker {
    visited_files: HashSet<PathBuf>,
    system_includes: HashSet<String>,
    library_blocks: Vec<CodeBlock>,
}

impl Default for TreeSitterShaker {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeSitterShaker {
    pub fn new() -> Self {
        Self {
            visited_files: HashSet::new(),
            system_includes: HashSet::new(),
            library_blocks: Vec::new(),
        }
    }

    fn collect_header(
        &mut self,
        file: &Path,
        resolver: &Resolver,
        parser: &mut Parser,
    ) -> Result<()> {
        let canon = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());
        if !self.visited_files.insert(canon.clone()) {
            return Ok(());
        }

        let source_bytes = fs::read(&canon)
            .with_context(|| format!("Failed to read header: {}", canon.display()))?;

        let tree = parser
            .parse(&source_bytes, None)
            .context("Tree-sitter failed to build syntax tree")?;

        let root = tree.root_node();
        let mut cursor = root.walk();

        for child in root.children(&mut cursor) {
            let kind = child.kind();

            if kind == "comment" {
                continue;
            }

            let block_text = child.utf8_text(&source_bytes).unwrap_or("").trim();

            if block_text.starts_with("#pragma once") {
                continue;
            }

            if let Some(inc) = parse_include(block_text) {
                if let Some(resolved) = resolver.resolve(&inc, &canon) {
                    self.collect_header(&resolved, resolver, parser)?;
                } else {
                    let formatted = if inc.is_quote {
                        format!("\"{}\"", inc.path)
                    } else {
                        format!("<{}>", inc.path)
                    };
                    self.system_includes.insert(formatted);
                }
                continue;
            }

            let always_keep = kind.starts_with("preproc_") || kind == "using_declaration";
            let defined_symbols = if always_keep {
                Vec::new()
            } else {
                get_declared_symbols(child, &source_bytes)
            };
            let referenced_symbols = get_referenced_symbols(child, &source_bytes);

            self.library_blocks.push(CodeBlock {
                raw_text: block_text.to_string(),
                defined_symbols,
                referenced_symbols,
                always_keep,
            });
        }

        Ok(())
    }
}

impl BundleStrategy for TreeSitterShaker {
    fn bundle(&mut self, entry: &Path, resolver: &Resolver) -> Result<String> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_cpp::LANGUAGE.into())
            .context("Failed to load Tree-Sitter C++ grammar")?;

        let entry_bytes = fs::read(entry)?;
        let entry_clean = strip_comments(&String::from_utf8_lossy(&entry_bytes));

        let mut main_body_lines = Vec::new();

        for line in entry_clean.lines() {
            let trimmed = line.trim();
            if let Some(inc) = parse_include(trimmed) {
                if let Some(resolved) = resolver.resolve(&inc, entry) {
                    self.collect_header(&resolved, resolver, &mut parser)?;
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
            if !trimmed.starts_with("#pragma once") {
                main_body_lines.push(line);
            }
        }

        let main_tree = parser
            .parse(&entry_bytes, None)
            .context("Failed to parse main entry file")?;
        let mut alive_symbols = get_referenced_symbols(main_tree.root_node(), &entry_bytes);

        let mut queue: VecDeque<String> = alive_symbols.iter().cloned().collect();
        while let Some(sym) = queue.pop_front() {
            for block in &self.library_blocks {
                if block.defined_symbols.contains(&sym) {
                    for r_sym in &block.referenced_symbols {
                        if alive_symbols.insert(r_sym.clone()) {
                            queue.push_back(r_sym.clone());
                        }
                    }
                }
            }
        }

        let mut out = String::new();
        out.push_str("// ====================================================================\n");
        out.push_str("// Bundled by Argonaut (Tree-Sitter AST Shaker)\n");
        out.push_str("// ====================================================================\n\n");

        let mut sys_sorted: Vec<_> = self.system_includes.iter().cloned().collect();
        sys_sorted.sort();
        for s in sys_sorted {
            out.push_str(&format!("#include {}\n", s));
        }
        out.push('\n');

        for block in &self.library_blocks {
            let is_alive = block.always_keep
                || block
                    .defined_symbols
                    .iter()
                    .any(|s| alive_symbols.contains(s));
            if is_alive && !block.raw_text.is_empty() {
                out.push_str(&block.raw_text);
                out.push_str("\n\n");
            }
        }

        out.push_str(&main_body_lines.join("\n"));
        Ok(out.replace("\n\n\n\n", "\n\n").replace("\n\n\n", "\n\n"))
    }
}

fn get_declared_symbols(node: Node, source: &[u8]) -> Vec<String> {
    let mut symbols = Vec::new();
    match node.kind() {
        "template_declaration" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() != "template_parameter_list" && child.kind() != "template" {
                    symbols.extend(get_declared_symbols(child, source));
                }
            }
        }
        "namespace_definition" => {
            if let Some(body) = node.child_by_field_name("body") {
                let mut cursor = body.walk();
                for child in body.children(&mut cursor) {
                    symbols.extend(get_declared_symbols(child, source));
                }
            }
        }
        "class_specifier" | "struct_specifier" | "enum_specifier" | "union_specifier"
        | "alias_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name")
                && let Ok(sym) = name_node.utf8_text(source)
            {
                symbols.push(sym.to_string());
            }
        }
        "function_definition" | "declaration" | "type_definition" => {
            if let Some(type_node) = node.child_by_field_name("type") {
                symbols.extend(get_declared_symbols(type_node, source));
            }
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if let Some(id) = extract_core_identifier(child, source) {
                    symbols.push(id);
                }
            }
        }
        _ => {}
    }
    symbols
}

fn extract_core_identifier(node: Node, source: &[u8]) -> Option<String> {
    match node.kind() {
        "identifier" | "type_identifier" | "field_identifier" | "destructor_name" => {
            node.utf8_text(source).ok().map(|s| s.to_string())
        }
        "init_declarator"
        | "function_declarator"
        | "pointer_declarator"
        | "reference_declarator"
        | "array_declarator"
        | "parenthesized_declarator" => node
            .child_by_field_name("declarator")
            .and_then(|c| extract_core_identifier(c, source)),
        _ => None,
    }
}

fn get_referenced_symbols(node: Node, source: &[u8]) -> HashSet<String> {
    let mut refs = HashSet::new();
    let mut stack = vec![node];

    while let Some(curr) = stack.pop() {
        if curr.kind() == "comment" {
            continue;
        }
        if curr.child_count() == 0 {
            if matches!(
                curr.kind(),
                "identifier" | "type_identifier" | "field_identifier" | "namespace_identifier"
            ) && let Ok(text) = curr.utf8_text(source)
            {
                refs.insert(text.to_string());
            }
        } else {
            let mut cursor = curr.walk();
            for child in curr.children(&mut cursor) {
                stack.push(child);
            }
        }
    }
    refs
}
