use super::super::*;
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
    is_macro: bool,
    namespaces: Vec<String>,
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

        let source_bytes = match fs::read(&canon) {
            Ok(bytes) => bytes,
            Err(err) => {
                let closure = || format!("Failed to read header: {}", canon.display());
                return Err(err).with_context(closure);
            }
        };

        let tree = parser
            .parse(&source_bytes, None)
            .context("Tree sitter failed to build syntax tree")?;

        let mut current_ns = Vec::new();
        self.extract_blocks(
            tree.root_node(),
            &source_bytes,
            resolver,
            &canon,
            parser,
            &mut current_ns,
        )?;

        Ok(())
    }

    fn extract_blocks<'a>(
        &mut self,
        node: Node<'a>,
        source: &[u8],
        resolver: &Resolver,
        canon: &Path,
        parser: &mut Parser,
        current_ns: &mut Vec<String>,
    ) -> Result<()> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if !child.is_named() {
                continue;
            }

            let kind = child.kind();

            if kind == "comment" {
                continue;
            }

            if kind == "namespace_definition" {
                let name = if let Some(name_node) = child.child_by_field_name("name") {
                    name_node.utf8_text(source).unwrap_or_default().to_string()
                } else {
                    String::new()
                };

                if let Some(body) = child.child_by_field_name("body") {
                    current_ns.push(name);
                    self.extract_blocks(body, source, resolver, canon, parser, current_ns)?;
                    current_ns.pop();
                }
                continue;
            }

            let mut end_byte = child.end_byte();
            let mut next_node = child.next_sibling();

            while let Some(n_node) = next_node {
                if !n_node.is_named() && n_node.kind() == ";" {
                    end_byte = n_node.end_byte();
                    next_node = n_node.next_sibling();
                } else {
                    break;
                }
            }

            let block_text = std::str::from_utf8(&source[child.start_byte()..end_byte])
                .unwrap_or_default()
                .trim();

            if block_text.starts_with("#pragma once") {
                continue;
            }

            if let Some(include) = parse_include(block_text) {
                if let Some(resolved) = resolver.resolve(&include, canon) {
                    self.collect_header(&resolved, resolver, parser)?;
                } else {
                    let formatted =
                        BundleUtilities::format_include(&include.path, include.is_quote);
                    self.system_includes.insert(formatted);
                }
                continue;
            }

            let macro_def = kind == "preproc_def" || kind == "preproc_function_def";
            let always_keep = kind.starts_with("preproc_") || kind == "using_declaration";

            let defined = BundleUtilities::declared_symbols(child, source);
            let referenced = BundleUtilities::referenced_symbols(child, source);

            self.library_blocks.push(CodeBlock {
                raw_text: block_text.to_string(),
                defined_symbols: defined,
                referenced_symbols: referenced,
                always_keep,
                is_macro: macro_def,
                namespaces: current_ns.clone(),
            });
        }
        Ok(())
    }

    fn parse_includes(
        &mut self,
        entry_clean: &str,
        entry_path: &Path,
        resolver: &Resolver,
        parser: &mut Parser,
    ) -> Result<Vec<String>> {
        let mut main_lines = Vec::new();

        for line in entry_clean.lines() {
            let trimmed = line.trim();
            if let Some(include) = parse_include(trimmed) {
                if let Some(resolved) = resolver.resolve(&include, entry_path) {
                    self.collect_header(&resolved, resolver, parser)?;
                } else {
                    let formatted =
                        BundleUtilities::format_include(&include.path, include.is_quote);
                    self.system_includes.insert(formatted);
                }
            } else if !trimmed.starts_with("#pragma once") {
                main_lines.push(line.to_string());
            }
        }

        Ok(main_lines)
    }

    fn find_alive(&self, entry_bytes: &[u8], parser: &mut Parser) -> Result<HashSet<String>> {
        let main_tree = parser
            .parse(entry_bytes, None)
            .context("Failed to parse main entry file")?;
        let mut alive = BundleUtilities::referenced_symbols(main_tree.root_node(), entry_bytes);
        for block in &self.library_blocks {
            if block.always_keep && !block.is_macro {
                for reference in &block.referenced_symbols {
                    alive.insert(reference.clone());
                }
            }
        }
        Ok(alive)
    }

    fn tree_shake(&self, mut alive: HashSet<String>) -> HashSet<String> {
        let mut queue: VecDeque<String> = alive.iter().cloned().collect();
        while let Some(symbol) = queue.pop_front() {
            for block in &self.library_blocks {
                if block.defined_symbols.contains(&symbol) {
                    for reference in &block.referenced_symbols {
                        if alive.insert(reference.clone()) {
                            queue.push_back(reference.clone());
                        }
                    }
                }
            }
        }
        alive
    }

    fn reassemble(&self, main_lines: &[String], alive: &HashSet<String>) -> String {
        let mut output = String::new();
        output
            .push_str("// ====================================================================\n");
        output.push_str("// Bundled by Argonaut (Tree-Sitter AST Shaker)\n");
        output.push_str(
            "// ====================================================================\n\n",
        );

        let mut sorted: Vec<_> = self.system_includes.iter().cloned().collect();
        sorted.sort();
        for symbol in sorted {
            output.push_str(&format!("#include {}\n", symbol));
        }
        output.push('\n');

        let mut current_ns: Vec<String> = Vec::new();

        for block in &self.library_blocks {
            let is_alive = block.always_keep
                || block
                    .defined_symbols
                    .iter()
                    .any(|symbol| alive.contains(symbol));

            if is_alive && !block.raw_text.is_empty() {
                let target_ns = &block.namespaces;

                let mut common_len = 0;

                for (c, t) in current_ns.iter().zip(target_ns.iter()) {
                    if c == t {
                        common_len += 1;
                    } else {
                        break;
                    }
                }

                while current_ns.len() > common_len {
                    current_ns.pop();
                    output.push_str("}\n");
                }

                for ns in target_ns.iter().skip(common_len) {
                    if ns.is_empty() {
                        output.push_str("namespace {\n");
                    } else {
                        output.push_str(&format!("namespace {} {{\n", ns));
                    }
                    current_ns.push(ns.clone());
                }

                output.push_str(&block.raw_text);
                output.push_str("\n\n");
            }
        }

        while current_ns.pop().is_some() {
            output.push_str("}\n");
        }

        output.push_str(&main_lines.join("\n"));
        output.replace("\n\n\n\n", "\n\n").replace("\n\n\n", "\n\n")
    }
}

impl BundleStrategy for TreeSitterShaker {
    fn bundle(&mut self, entry: &Path, resolver: &Resolver) -> Result<String> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_cpp::LANGUAGE.into())
            .context("Failed to load tree-sitter C++ grammar")?;

        let entry_bytes = fs::read(entry)?;
        let entry_clean = strip_comments(&String::from_utf8_lossy(&entry_bytes));

        let body_lines = self.parse_includes(&entry_clean, entry, resolver, &mut parser)?;
        let alive_symbols = self.find_alive(&entry_bytes, &mut parser)?;
        let alive_symbols = self.tree_shake(alive_symbols);
        let output = self.reassemble(&body_lines, &alive_symbols);
        Ok(output)
    }
}

struct BundleUtilities;

impl BundleUtilities {
    fn format_include(path: &str, is_quote: bool) -> String {
        if is_quote {
            format!("\"{path}\"")
        } else {
            format!("<{path}>")
        }
    }

    fn declared_symbols(node: Node, source: &[u8]) -> Vec<String> {
        let mut symbols = Vec::new();
        let field_text = |node: Node, field: &str, source: &[u8]| {
            let field_node = node.child_by_field_name(field)?;
            let text = field_node.utf8_text(source).ok()?;
            Some(text.to_owned())
        };
        match node.kind() {
            "preproc_def" | "preproc_function_def" => {
                if let Some(symbol) = field_text(node, "name", source) {
                    symbols.push(symbol);
                }
            }
            "template_declaration" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() != "template_parameter_list" && child.kind() != "template" {
                        symbols.extend(Self::declared_symbols(child, source));
                    }
                }
            }
            "namespace_definition" => {
                let body = node.child_by_field_name("body");

                if let Some(body) = body {
                    let mut cursor = body.walk();
                    for child in body.children(&mut cursor) {
                        symbols.extend(Self::declared_symbols(child, source));
                    }
                }
            }
            "class_specifier" | "struct_specifier" | "enum_specifier" | "union_specifier"
            | "alias_declaration" => {
                if let Some(symbol) = field_text(node, "name", source) {
                    symbols.push(symbol);
                }
            }
            "function_definition" | "declaration" | "type_definition" => {
                let type_node = node.child_by_field_name("type");
                if let Some(type_node) = type_node {
                    symbols.extend(Self::declared_symbols(type_node, source));
                }
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if let Some(identifier) = Self::extract_core(child, source) {
                        symbols.push(identifier);
                    }
                }
            }
            _ => {}
        }
        symbols
    }

    fn extract_core(node: Node, source: &[u8]) -> Option<String> {
        match node.kind() {
            "identifier" | "type_identifier" | "field_identifier" | "destructor_name" => {
                node.utf8_text(source).ok().map(|str| str.to_string())
            }
            "init_declarator"
            | "function_declarator"
            | "pointer_declarator"
            | "reference_declarator"
            | "array_declarator"
            | "parenthesized_declarator" => node
                .child_by_field_name("declarator")
                .and_then(|character| Self::extract_core(character, source)),
            _ => None,
        }
    }

    fn referenced_symbols(node: Node, source: &[u8]) -> HashSet<String> {
        let mut refs = HashSet::new();
        let mut stack = vec![node];

        while let Some(current) = stack.pop() {
            if current.kind() == "comment" {
                continue;
            }

            if current.kind() == "preproc_arg" {
                if let Ok(text) = current.utf8_text(source) {
                    Self::extract_identifiers(text, &mut refs);
                }
                continue;
            }

            if current.child_count() == 0 {
                let is_identifier: bool = matches!(
                    current.kind(),
                    "identifier" | "type_identifier" | "field_identifier" | "namespace_identifier"
                );
                if is_identifier && let Ok(text) = current.utf8_text(source) {
                    refs.insert(text.to_string());
                }
            } else {
                let mut cursor = current.walk();
                for child in current.children(&mut cursor) {
                    stack.push(child);
                }
            }
        }
        refs
    }

    fn extract_identifiers(text: &str, refs: &mut HashSet<String>) {
        let mut current_word = String::new();
        for character in text.chars() {
            if character.is_alphanumeric() || character == UNDERSCORE {
                current_word.push(character);
            } else {
                if !current_word.is_empty() {
                    if let Some(first_character) = current_word.chars().next()
                        && (first_character.is_alphabetic() || first_character == UNDERSCORE) {
                            refs.insert(current_word.clone());
                        }
                    current_word.clear();
                }
            }
        }

        if !current_word.is_empty()
            && let Some(first_character) = current_word.chars().next()
                && (first_character.is_alphabetic() || first_character == UNDERSCORE) {
                    refs.insert(current_word);
                }
    }
}
