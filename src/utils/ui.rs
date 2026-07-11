use colored::Colorize;
use std::fmt::Display;

pub struct Ui;

impl Ui {
    pub fn section<S: Display>(title: S) {
        println!(
            "\n{} {}\n",
            "::".bold().cyan(),
            title.to_string().bold().cyan()
        );
    }

    pub fn info<S: Display>(msg: S) {
        println!("  {}  {}", "".cyan(), msg);
    }

    pub fn ok<S: Display>(msg: S) {
        println!("  {}  {}", "✔".green(), msg);
    }

    pub fn fail<S: Display>(msg: S) {
        println!("  {}  {}", "✘".red(), msg);
    }

    pub fn warn<S: Display>(msg: S) {
        println!("  {}  {}", "⚠".yellow(), msg);
    }

    pub fn meta<K: Display, V: Display>(key: K, value: V) {
        println!("  {:<12} {}", format!("[{}]", key).dimmed(), value);
    }

    pub fn time(nanos: u128) {
        let ms = nanos as f64 / 1_000_000.0;
        println!("  {}  {}", "⏱".dimmed(), format!("{ms:.4} ms").dimmed());
    }

    pub fn test_result(id: &str, code: &str, time_ms: f64, desc: &str) {
        let (color_code, color_desc) = match code {
            "AC" => (code.green().bold(), desc.green()),
            "WA" => (code.red().bold(), desc.red()),
            "RTE" => (code.yellow().bold(), desc.yellow()),
            _ => (code.magenta().bold(), desc.magenta()),
        };

        println!(
            "  [{}] {:<3} {} {}",
            id.cyan(),
            color_code,
            format!("({:.2} ms)", time_ms).dimmed(),
            color_desc
        );
    }

    pub fn test_diff(actual: &str, expected: &str) {
        let actual_trunc = Self::truncate(actual.trim(), 100);
        let expected_trunc = Self::truncate(expected.trim(), 100);

        println!("      {} {}", "Expected:".dimmed(), expected_trunc);
        println!("      {} {}", "Actual:  ".dimmed(), actual_trunc);
    }

    pub fn test_summary(passed: usize, total: usize) {
        println!(
            "\n  Passed {} / {} tests",
            passed.to_string().cyan(),
            total.to_string().cyan()
        );
    }

    fn truncate(s: &str, max_chars: usize) -> String {
        let mut flat = s.replace('\n', " ↵ ");
        if flat.chars().count() > max_chars {
            let end: usize = flat
                .char_indices()
                .map(|(i, _)| i)
                .nth(max_chars)
                .unwrap_or(flat.len());
            flat.truncate(end);
            flat.push_str("...");
        }
        flat
    }
}
