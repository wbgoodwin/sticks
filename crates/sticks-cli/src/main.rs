//! The `sticks` command: the same flow as the app, driven by a project file.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use sticks_core::Project;

#[derive(Parser)]
#[command(
    version,
    about = "Frame small structures from a plain-text project file"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Load a project file and summarize what it describes.
    Check { project: PathBuf },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Check { project } => check(&project),
    }
}

fn check(path: &PathBuf) -> ExitCode {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("error: can't read {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
    };
    let project = match Project::from_toml(&text) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
    };

    println!("{}", project.name);
    println!(
        "  studs {:#} on center, kerf {}",
        project.rules.on_center, project.rules.kerf
    );
    for wall in &project.walls {
        println!(
            "  wall {}: {} long, {} tall, {} opening(s)",
            wall.id,
            wall.length,
            wall.height,
            wall.openings.len()
        );
    }
    ExitCode::SUCCESS
}
