//! The `sticks` command: the same flow as the app, driven by a project file.

mod text;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use sticks_core::{Member, Project};
use sticks_cutlist::{cut_list, CutList};
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
    /// Plan which stock boards to buy and how to cut every piece from them.
    Cuts { project: PathBuf },
    /// Write the shopping list and cut list as CSV files.
    Build {
        project: PathBuf,
        /// Directory to write into; created if missing.
        #[arg(long, default_value = "build")]
        out: PathBuf,
    },
}

fn main() -> ExitCode {
    let result = match Cli::parse().command {
        Command::Check { project } => load(&project).map(|p| check(&p)),
        Command::Frame { project, wall } => load(&project).and_then(|p| frame(&p, wall.as_deref())),
        Command::Cuts { project } => load(&project)
            .and_then(|p| plan(&p))
            .map(|cl| print!("{}", text::cut_list(&cl))),
        Command::Build { project, out } => load(&project).and_then(|p| build(&p, &out)),
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

/// Every member of every wall, framed with the project's rules.
fn members(project: &Project) -> Result<Vec<Member>, String> {
    let mut out = Vec::new();
    for wall in &project.walls {
        let framed = frame_wall(wall, &project.rules).map_err(|e| e.to_string())?;
        out.extend(framed.members.into_iter().map(|p| p.member));
    }
    Ok(out)
}

fn plan(project: &Project) -> Result<CutList, String> {
    cut_list(&members(project)?, &project.stock, &project.rules).map_err(|e| e.to_string())
}

fn build(project: &Project, out: &Path) -> Result<(), String> {
    let cl = plan(project)?;
    std::fs::create_dir_all(out).map_err(|e| format!("can't create {}: {e}", out.display()))?;
    for (name, body) in [
        ("bom.csv", sticks_export::bom_csv(&cl)),
        ("cut-list.csv", sticks_export::cut_list_csv(&cl)),
    ] {
        let path = out.join(name);
        std::fs::write(&path, body).map_err(|e| format!("can't write {}: {e}", path.display()))?;
        println!("wrote {}", path.display());
    }
    for p in &cl.too_long {
        eprintln!(
            "note: {} ({} {}, {}) is longer than any {} stock and isn't in the cut list",
            p.label,
            p.size,
            p.kind.name(),
            p.length,
            p.size
        );
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
