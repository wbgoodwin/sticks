//! Design changes in plain English, turned into typed edits.
//!
//! Rust reads the notation (clauses, lengths, pitches, lumber sizes) and asks
//! a classifier such as Jev a fixed batch of multiple-choice questions about
//! the meaning ([`questions`]). Confident answers become typed edits; the
//! rest come back as a [`Question`] for the user. Answers go back in by
//! question id and the request is read again, so nothing is kept between
//! calls. Rust then places, checks and frames the result.

mod design;
mod edit;
mod interpret;
mod words;

pub use design::{design, Design};
pub use edit::{Dimension, Edit, EditError, Framing, Side};
pub use interpret::{
    interpret, questions, Answer, Answers, Ask, Choice, Interpretation, Question, Step, CONFIDENT,
};

use edit::kind_name;
use sticks_core::{Length, OpeningKind, Project, RoofKind};
use sticks_rules::frame_project;

/// The project with `edits` applied, in order.
pub fn apply_all(project: &Project, edits: &[Edit]) -> Result<Project, EditError> {
    let mut p = project.clone();
    for e in edits {
        e.apply(&mut p)?;
    }
    Ok(p)
}

/// Why a project won't frame, if it won't.
pub fn framing_problems(project: &Project) -> Vec<String> {
    match frame_project(project) {
        Ok(f) => f.errors().iter().map(|e| e.to_string()).collect(),
        Err(e) => vec![e.to_string()],
    }
}

/// The design in a few lines of builder English, as `sticks check` prints
/// it. The classifier reads [`design`] instead.
pub fn describe(project: &Project) -> String {
    let mut out = format!("{}\n", project.name);
    out += &format!(
        "  studs {:#} on center, kerf {}\n",
        project.rules.on_center, project.rules.kerf
    );
    if let Some(b) = &project.building {
        out += &format!(
            "  building: {} wide, {} deep, {} tall at the front\n",
            b.width, b.depth, b.height
        );
    }
    if let Some(f) = &project.floor {
        out += &format!(
            "  floor: {} joists on {} {} beams\n",
            f.joist, f.beams, f.beam
        );
    }
    if let Some(r) = &project.roof {
        let kind = match r.kind {
            RoofKind::LeanTo => "lean-to",
            RoofKind::Gable => "gable",
        };
        out += &format!("  roof: {kind} at {}, {} rafters\n", r.pitch, r.rafter);
    }
    if let Some(p) = &project.porch {
        let deck = if p.deck.is_some() { " with a deck" } else { "" };
        out += &format!("  porch: {} deep on {} posts{deck}\n", p.depth, p.post);
    }
    if let Some(s) = &project.sheathing {
        let t = |l: Option<Length>| l.map_or("none".to_string(), |l| format!("{l:#}"));
        out += &format!("  sheathing: walls {}, roof {}\n", t(s.wall), t(s.roof));
    }
    for wall in &project.walls {
        let size = match (wall.length, wall.height) {
            (Some(l), Some(h)) => format!("{l} long, {h} tall"),
            _ => "sized by the building".to_string(),
        };
        let openings: Vec<String> = wall
            .openings
            .iter()
            .map(|o| {
                format!(
                    "a {} {} x {}, {} from its left end",
                    kind_name(o.kind),
                    o.width,
                    o.height,
                    o.offset
                )
            })
            .collect();
        let has = if openings.is_empty() {
            "no openings".to_string()
        } else {
            openings.join("; ")
        };
        out += &format!("  wall {}: {size}; {has}\n", wall.id);
    }
    // Totals, so "the door" can be read as the only door.
    let count = |kind| {
        project
            .walls
            .iter()
            .flat_map(|w| &w.openings)
            .filter(|o| o.kind == kind)
            .count()
    };
    let all: Vec<String> = [
        OpeningKind::Door,
        OpeningKind::Window,
        OpeningKind::Penetration,
    ]
    .into_iter()
    .map(|kind| match count(kind) {
        0 => format!("no {}s", kind_name(kind)),
        1 => format!("1 {}", kind_name(kind)),
        n => format!("{n} {}s", kind_name(kind)),
    })
    .collect();
    out += &format!("  in all: {}\n", all.join(", "));
    out
}
