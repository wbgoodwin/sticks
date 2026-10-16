//! The `sticks` command: the same flow as the app, driven by a project file.

mod text;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use sticks_core::Project;
use sticks_rules::frame_wall;

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
    /// Frame each wall and print its members and layout marks.
    Frame {
        project: PathBuf,
        /// Only this wall (by id).
        #[arg(long)]
        wall: Option<String>,
    },
}

fn main() -> ExitCode {
    let result = match Cli::parse().command {
        Command::Check { project } => load(&project).map(|p| check(&p)),
        Command::Frame { project, wall } => load(&project).and_then(|p| frame(&p, wall.as_deref())),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn load(path: &Path) -> Result<Project, String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("can't read {}: {e}", path.display()))?;
    Project::from_toml(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn frame(project: &Project, only: Option<&str>) -> Result<(), String> {
    let walls: Vec<_> = project
        .walls
        .iter()
        .filter(|w| only.is_none_or(|id| w.id == id))
        .collect();
    if let (Some(id), true) = (only, walls.is_empty()) {
        return Err(format!("no wall with id {id:?}"));
    }
    for wall in walls {
        let framed = frame_wall(wall, &project.rules).map_err(|e| e.to_string())?;
        print!("{}", text::wall(&framed, project.rules.layout_from));
    }
    Ok(())
}

fn check(project: &Project) {
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
}
