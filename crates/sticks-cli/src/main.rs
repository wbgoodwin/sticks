//! The `sticks` command: the same flow as the app, driven by a project file.

mod text;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use sticks_core::Project;
use sticks_cutlist::{cut_list_with_sheets, CutList};
use sticks_rules::{frame_project, FramedProject};

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
    /// Frame the walls, floor and roof and print their members and layout marks.
    Frame {
        project: PathBuf,
        /// Only this part: a wall's id, `floor` or `roof`.
        #[arg(long)]
        part: Option<String>,
    },
    /// Plan which stock boards to buy and how to cut every piece from them.
    Cuts { project: PathBuf },
    /// Write the shopping list and cut list as CSV, an SVG drawing of every
    /// wall, the floor and the roof, and a printable build packet.
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
        Command::Frame { project, part } => load(&project).and_then(|p| frame(&p, part.as_deref())),
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

/// Frames the whole project, failing on the first part that doesn't frame.
fn framed(project: &Project) -> Result<FramedProject, String> {
    let f = frame_project(project).map_err(|e| e.to_string())?;
    match f.errors().first() {
        Some(e) => Err(e.to_string()),
        None => Ok(f),
    }
}

fn frame(project: &Project, only: Option<&str>) -> Result<(), String> {
    let f = framed(project)?;
    let from = project.rules.layout_from;
    let want = |name: &str| only.is_none_or(|p| p == name);
    let mut shown = false;
    for w in f.walls.iter().flatten().filter(|w| want(&w.id)) {
        print!("{}", text::wall(w, from));
        shown = true;
    }
    if let Some(Ok(floor)) = f.floor.as_ref().filter(|_| want("floor")) {
        print!("{}", text::floor(floor, from));
        shown = true;
    }
    if let Some(Ok(roof)) = f.roof.as_ref().filter(|_| want("roof")) {
        print!("{}", text::roof(roof, from));
        shown = true;
    }
    match (only, shown) {
        (Some(part), false) => Err(format!("no wall, floor or roof called {part:?}")),
        _ => Ok(()),
    }
}

fn plan(project: &Project) -> Result<CutList, String> {
    plan_framed(project, &framed(project)?)
}

fn plan_framed(project: &Project, f: &FramedProject) -> Result<CutList, String> {
    cut_list_with_sheets(
        &f.members(),
        &f.sheet_pieces(),
        &project.stock,
        &project.sheets,
        &project.rules,
    )
    .map_err(|e| e.to_string())
}

fn build(project: &Project, out: &Path) -> Result<(), String> {
    let f = framed(project)?;
    let cl = plan_framed(project, &f)?;
    let from = project.rules.layout_from;
    let name = &project.name;
    let mut files = vec![
        ("bom.csv".to_string(), sticks_export::bom_csv(&cl)),
        ("cut-list.csv".to_string(), sticks_export::cut_list_csv(&cl)),
    ];
    for w in f.walls.iter().flatten() {
        files.push((
            format!("wall-{}.svg", file_safe(&w.id)),
            sticks_export::wall_svg(name, w, from),
        ));
    }
    if let Some(Ok(floor)) = &f.floor {
        files.push((
            "floor.svg".to_string(),
            sticks_export::floor_svg(name, floor, from),
        ));
        files.push((
            "subfloor.svg".to_string(),
            sticks_export::subfloor_svg(name, floor),
        ));
    }
    if let Some(Ok(roof)) = &f.roof {
        files.push((
            "roof.svg".to_string(),
            sticks_export::roof_svg(name, roof, from),
        ));
        files.push((
            "rafter.svg".to_string(),
            sticks_export::rafter_svg(name, roof),
        ));
    }
    files.push((
        "print.html".to_string(),
        sticks_export::print_html(project, &f, &cl),
    ));

    std::fs::create_dir_all(out).map_err(|e| format!("can't create {}: {e}", out.display()))?;
    for (file, body) in files {
        let path = out.join(file);
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
    eprintln!("note: {}", sticks_export::NOTICE);
    Ok(())
}

/// A wall id as a file name: letters, digits, `-` and `_` kept.
fn file_safe(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn check(project: &Project) {
    println!("{}", project.name);
    println!(
        "  studs {:#} on center, kerf {}",
        project.rules.on_center, project.rules.kerf
    );
    if let Some(b) = &project.building {
        println!(
            "  building: {} wide, {} deep, {} tall at the front",
            b.width, b.depth, b.height
        );
    }
    if let Some(f) = &project.floor {
        println!(
            "  floor: {} joists on {} {} beams",
            f.joist, f.beams, f.beam
        );
    }
    if let Some(r) = &project.roof {
        println!("  roof: lean-to at {}, {} rafters", r.pitch, r.rafter);
    }
    for wall in &project.walls {
        let size = match (wall.length, wall.height) {
            (Some(l), Some(h)) => format!("{l} long, {h} tall"),
            _ => "sized by the building".to_string(),
        };
        println!(
            "  wall {}: {size}, {} opening(s)",
            wall.id,
            wall.openings.len()
        );
    }
}
