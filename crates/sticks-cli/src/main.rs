//! The `sticks` command: the same flow as the app, driven by a project file.

mod edit;
mod text;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use sticks_core::Project;
use sticks_cutlist::{cut_list_with_sheets, CutList};
use sticks_hardware::HardwareList;
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
    Check {
        project: PathBuf,
        /// Print the design as JSON, as a classifier such as Jev reads it.
        #[arg(long)]
        json: bool,
    },
    /// Frame the walls, floor and roof and print their members and layout marks.
    Frame {
        project: PathBuf,
        /// Only this part: a wall's id, `floor`, `roof`, `porch`, `sheathing` or `deck`.
        #[arg(long)]
        part: Option<String>,
    },
    /// Plan which stock boards to buy and how to cut every piece from them.
    Cuts { project: PathBuf },
    /// Write the shopping list and cut list as CSV, an SVG drawing of every
    /// wall, the floor, roof, porch and sheathing, and a printable build
    /// packet.
    Build {
        project: PathBuf,
        /// Directory to write into; created if missing.
        #[arg(long, default_value = "build")]
        out: PathBuf,
    },
    /// Change the design with a plain-English request, such as
    /// "make it 20' wide and add a window on the left wall", and preview it.
    Edit {
        project: PathBuf,
        request: String,
        /// Print the questions a classifier such as Jev should answer, as
        /// JSON, and stop.
        #[arg(long)]
        questions: bool,
        /// A classifier's answers: JSON mapping question ids to
        /// `{"pick": ..., "confidence": ...}`.
        #[arg(long = "answers", value_name = "FILE")]
        answers_file: Option<PathBuf>,
        /// Your answer to a question, as `ID=LABEL`. Settles it.
        #[arg(long = "answer", value_name = "ID=LABEL")]
        answers: Vec<String>,
        /// Print the reading of the request and the preview as JSON.
        #[arg(long)]
        json: bool,
        /// Save the changed design back to the project file.
        #[arg(long)]
        write: bool,
    },
}

fn main() -> ExitCode {
    let result = match Cli::parse().command {
        Command::Check { project, json } => load(&project).and_then(|p| check(&p, json)),
        Command::Frame { project, part } => load(&project).and_then(|p| frame(&p, part.as_deref())),
        Command::Cuts { project } => load(&project).and_then(|p| {
            let f = framed(&p)?;
            let cl = plan_framed(&p, &f)?;
            let hw = hardware(&p, &f);
            print!("{}", text::cut_list(&cl, &f.on_hand_names(&p.on_hand), &hw));
            Ok(())
        }),
        Command::Build { project, out } => load(&project).and_then(|p| build(&p, &out)),
        Command::Edit {
            project,
            request,
            questions,
            answers_file,
            answers,
            json,
            write,
        } => load(&project).and_then(|p| {
            let opts = edit::Opts {
                questions,
                answers_file,
                answers,
                json,
                write,
            };
            edit::run(&p, &project, &request, &opts)
        }),
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
    if let Some(Ok(porch)) = f.porch.as_ref().filter(|_| want("porch")) {
        print!("{}", text::porch(porch, from));
        shown = true;
    }
    if let Some(Ok(s)) = f.sheathing.as_ref().filter(|_| want("sheathing")) {
        print!("{}", text::sheathing(s));
        shown = true;
    }
    if let Some(Ok(d)) = f.deck.as_ref().filter(|_| want("deck")) {
        print!("{}", text::deck(d, from));
        shown = true;
    }
    match (only, shown) {
        (Some(part), false) => Err(format!(
            "no wall, floor, roof, porch, sheathing or deck called {part:?}"
        )),
        _ => Ok(()),
    }
}

pub(crate) fn plan(project: &Project) -> Result<CutList, String> {
    plan_framed(project, &framed(project)?)
}

/// What to buy: every section but the ones the project has on hand.
fn plan_framed(project: &Project, f: &FramedProject) -> Result<CutList, String> {
    let (buy, _) = f.split(&project.on_hand);
    cut_list_with_sheets(
        &buy.members,
        &buy.sheets,
        &project.stock,
        &project.sheets,
        &project.rules,
    )
    .map_err(|e| e.to_string())
}

/// Connectors for every section but the ones the project has on hand.
fn hardware(project: &Project, f: &FramedProject) -> HardwareList {
    let (buy, _) = f.split(&project.on_hand);
    sticks_hardware::for_project(
        &buy.joints,
        sticks_rules::hardware_settings(project).as_ref(),
    )
}

fn build(project: &Project, out: &Path) -> Result<(), String> {
    let f = framed(project)?;
    let cl = plan_framed(project, &f)?;
    let hw = hardware(project, &f);
    let from = project.rules.layout_from;
    let name = &project.name;
    let mut files = vec![
        ("bom.csv".to_string(), sticks_export::bom_csv(&cl)),
        ("cut-list.csv".to_string(), sticks_export::cut_list_csv(&cl)),
        ("hardware.csv".to_string(), sticks_export::hardware_csv(&hw)),
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
        for (i, r) in roof.rafters.iter().enumerate() {
            let file = if i == 0 {
                "rafter.svg".to_string()
            } else {
                format!("rafter-{}.svg", i + 1)
            };
            files.push((file, sticks_export::rafter_svg(name, r)));
        }
    }
    if let Some(Ok(porch)) = &f.porch {
        files.push((
            "porch.svg".to_string(),
            sticks_export::porch_svg(name, porch, from),
        ));
        files.push((
            "porch-rafter.svg".to_string(),
            sticks_export::rafter_svg(name, &porch.rafter),
        ));
        if let Some(svg) = sticks_export::deck_svg(name, porch, from) {
            files.push(("deck.svg".to_string(), svg));
        }
    }
    if let Some(Ok(s)) = &f.sheathing {
        for (file, roofs) in [("wall-sheathing.svg", false), ("roof-sheathing.svg", true)] {
            if let Some(svg) = sticks_export::sheathing_svg(name, s, roofs) {
                files.push((file.to_string(), svg));
            }
        }
    }
    files.push((
        "print.html".to_string(),
        sticks_export::print_html(project, &f, &cl, &hw),
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

fn check(project: &Project, json: bool) -> Result<(), String> {
    if json {
        let design = sticks_intent::design(project);
        println!(
            "{}",
            serde_json::to_string_pretty(&design).map_err(|e| e.to_string())?
        );
    } else {
        print!("{}", sticks_intent::describe(project));
    }
    Ok(())
}
