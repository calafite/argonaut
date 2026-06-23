use clap::Parser;

use argonaut::cli::app::Cli;
use argonaut::utils::ui::Ui;

fn main() {
    if let Err(e) = Cli::parse().execute() {
        Ui::fail(e.to_string());
        std::process::exit(1);
    }
}
