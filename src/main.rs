mod app;
mod args;
mod editing;
mod integrations;

use clap::Parser;

fn main() {
    let cli = args::Cli::parse();
    if let Err(err) = app::run(cli.command) {
        eprintln!("Error: {err:#}");
        std::process::exit(1);
    }
}
