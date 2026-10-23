use crate::core::runner::{Runner, RunnerFlags};
use crate::parser::payload::TestCase;
use crate::utils::paths::ProjectLayout;
use crate::utils::ui::Ui;
use anyhow::{Context, Result};
use colored::Colorize;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::Path;

pub struct Tester;

impl Tester {
    pub fn execute_test(target: Option<&str>, force_input: bool, no_input: bool) -> Result<()> {
        let (binary, display) = crate::core::compiler::Compiler::resolve_test_target(target)?;
        Ui::section("Running Tests");
        Ui::meta("target", display);
        let tests_run = Self::run_suite(&binary)?;
        if tests_run > 0 {
            return Ok(());
        }
        let use_file = Runner::resolve_input(&binary, force_input, no_input)?;
        Runner::run(&binary, use_file)
    }

    pub fn save_tests(problem_name: &str, tests: &[TestCase]) -> Result<()> {
        let layout = ProjectLayout::new(problem_name);
        let test_directory = layout.tests_dir(problem_name);

        let closure = || {
            format!(
                "Failed to create test directory layout: {}",
                test_directory.display()
            )
        };
        fs::create_dir_all(&test_directory).with_context(closure)?;

        for (index, test) in tests.iter().enumerate() {
            let input = test_directory.join(format!("in_{}.txt", index + 1));
            let output = test_directory.join(format!("out_{}.txt", index + 1));

            let closure = || format!("Failed to write input payload: {}", input.display());
            fs::write(&input, &test.input).with_context(closure)?;

            let closure = || format!("Failed to write output payload: {}", output.display());
            fs::write(&output, &test.output).with_context(closure)?;
        }

        Ok(())
    }

    pub fn run_suite(binary: &Path) -> Result<usize> {
        let stem = binary.file_stem().unwrap_or_default().to_string_lossy();
        let layout = ProjectLayout::from_binary(binary);
        let test_directory = layout.tests_dir(&stem);

        if !test_directory.exists() || !test_directory.is_dir() {
            return Ok(0);
        }

        let mut test_cases = Vec::new();
        if let Ok(entries) = fs::read_dir(&test_directory) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with("in_") && name.ends_with(".txt") {
                    let id = name
                        .trim_start_matches("in_")
                        .trim_end_matches(".txt")
                        .to_string();
                    let output_file = test_directory.join(format!("out_{}.txt", id));
                    if output_file.exists() {
                        test_cases.push((id, entry.path(), output_file));
                    }
                }
            }
        }

        if test_cases.is_empty() {
            return Ok(0);
        }

        test_cases.sort_by_key(|(id, _, _)| id.parse::<u32>().unwrap_or(0));

        let memory_limit_mb = Self::find_memory_limit(binary);
        let flags = RunnerFlags {
            memory_limit_mb,
            ..RunnerFlags::default()
        };

        println!();
        let mut passed = 0;
        let total = test_cases.len();

        for (id, in_file, out_file) in &test_cases {
            let expected = fs::read_to_string(out_file)?;

            match Runner::execute_captured(binary, in_file, flags) {
                Ok(result) => {
                    let time_ms = result.time_nanos as f64 / 1_000_000.0;

                    if !result.status.success() {
                        Ui::test_result(id, "RTE", time_ms, "Runtime Error");
                        if !result.stderr.is_empty() {
                            println!("{}", result.stderr.trim().dimmed());
                        }
                    } else if Self::validate_output(&result.stdout, &expected) {
                        Ui::test_result(id, "AC", time_ms, "Accepted");
                        passed += 1;
                    } else {
                        Ui::test_result(id, "WA", time_ms, "Wrong Answer");
                        Ui::test_diff(&result.stdout, &expected);
                    }
                }
                Err(error) => {
                    Ui::test_result(id, "ERR", 0.0, "Execution Failed");
                    println!("  {}", error.to_string().red());
                }
            }
        }

        Ui::test_summary(passed, total);
        Ok(total)
    }

    fn validate_output(actual: &str, expected: &str) -> bool {
        let actual_tokens = actual.split_whitespace();
        let expected_tokens = expected.split_whitespace();
        actual_tokens.eq(expected_tokens)
    }

    fn find_memory_limit(binary: &Path) -> Option<usize> {
        let stem = binary.file_stem()?.to_string_lossy();
        let layout = ProjectLayout::from_binary(binary);
        let source_file = layout.source_file(&stem);

        if !source_file.exists() {
            return None;
        }

        let file = File::open(source_file).ok()?;
        let reader = BufReader::new(file);

        for line in reader.lines().map_while(Result::ok) {
            if line.contains("Memory Limit:") {
                let parts: Vec<&str> = line.split("Memory Limit:").collect();
                if let Some(limit_part) = parts.get(1) {
                    let num_part: String = limit_part
                        .chars()
                        .skip_while(|c| c.is_whitespace())
                        .take_while(|c| c.is_ascii_digit())
                        .collect();
                    if let Ok(limit_mb) = num_part.parse::<usize>() {
                        return Some(limit_mb);
                    }
                }
            }
        }
        None
    }
}
