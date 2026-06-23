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
}
