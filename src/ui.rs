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
        println!("  {} {}", "ℹ".cyan(), msg);
    }

    pub fn ok<S: Display>(msg: S) {
        println!("  {} {}", "✔".green(), msg);
    }

    pub fn fail<S: Display>(msg: S) {
        println!("  {} {}", "✘".red(), msg);
    }

    pub fn warn<S: Display>(msg: S) {
        println!("  {} {}", "⚠".yellow(), msg);
    }

    pub fn meta<K: Display, V: Display>(key: K, value: V) {
        println!("  {:<12} {}", format!("[{}]", key).dimmed(), value);
    }

    pub fn time(duration: std::time::Duration) {
        println!(
            "\n  {}",
            format!("real: {:.3} sec", duration.as_secs_f64()).dimmed()
        );
    }
}
