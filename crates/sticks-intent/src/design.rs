//! The design as a classifier such as Jev reads it with each request: the
//! same shape for every project, in builder notation, with each door and
//! window labeled the way the questions' options name it (`front-1`).

use serde::Serialize;
use sticks_core::{OpeningKind, Project, RoofKind};

use crate::edit::{kind_name, Side};

#[derive(Debug, Serialize)]
pub struct Design {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub building: Option<Building>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roof: Option<Roof>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub floor: Option<Floor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub porch: Option<Porch>,
    pub framing: Framing,
    pub walls: Vec<Wall>,
    /// How many of each kind of opening there are in all.
    pub openings: Totals,
}

#[derive(Debug, Serialize)]
pub struct Building {
    /// Along the front wall.
    pub width: String,
    /// Front to back.
    pub depth: String,
    /// At the front wall.
    pub height: String,
}

#[derive(Debug, Serialize)]
pub struct Roof {
    pub shape: String,
    pub pitch: String,
    pub rafters: String,
    pub front_overhang: String,
    pub back_overhang: String,
}

#[derive(Debug, Serialize)]
pub struct Floor {
    pub joists: String,
    pub beams: String,
}

#[derive(Debug, Serialize)]
pub struct Porch {
    pub depth: String,
    pub deck: bool,
}

#[derive(Debug, Serialize)]
pub struct Framing {
    pub studs: String,
    /// Studs, joists and rafters, on center.
    pub spacing: String,
}

#[derive(Debug, Serialize)]
pub struct Wall {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length: Option<String>,
    pub openings: Vec<Opening>,
}

#[derive(Debug, Serialize)]
pub struct Opening {
    /// The label the questions use for it, like `front-1`.
    pub id: String,
    pub kind: &'static str,
    pub width: String,
    pub height: String,
    /// From the wall's left end, seen from outside, to its left side.
    pub from_left_end: String,
    /// Bottom of the opening above the floor, for windows and holes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub above_floor: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Totals {
    pub doors: usize,
    pub windows: usize,
    pub penetrations: usize,
}

/// The design as the classifier reads it.
pub fn design(project: &Project) -> Design {
    let b = project.building.as_ref();
    // A building has its four walls whether or not the file lists them.
    let walls: Vec<Wall> = match b {
        Some(b) => Side::ALL
            .iter()
            .map(|s| {
                let openings = project
                    .walls
                    .iter()
                    .filter(|w| w.id == s.id())
                    .flat_map(|w| &w.openings);
                wall(s.id(), Some(s.length(b).to_string()), openings)
            })
            .collect(),
        None => project
            .walls
            .iter()
            .map(|w| wall(&w.id, w.length.map(|l| l.to_string()), &w.openings))
            .collect(),
    };
    let count = |kind: &str| {
        walls
            .iter()
            .flat_map(|w| &w.openings)
            .filter(|o| o.kind == kind)
            .count()
    };
    let openings = Totals {
        doors: count("door"),
        windows: count("window"),
        penetrations: count("penetration"),
    };
    Design {
        name: project.name.clone(),
        building: b.map(|b| Building {
            width: b.width.to_string(),
            depth: b.depth.to_string(),
            height: b.height.to_string(),
        }),
        roof: project.roof.as_ref().map(|r| Roof {
            shape: match r.kind {
                RoofKind::LeanTo => "lean-to (one slope)",
                RoofKind::Gable => "gable (two slopes)",
            }
            .into(),
            pitch: r.pitch.to_string(),
            rafters: r.rafter.to_string(),
            front_overhang: format!("{:#}", r.front_overhang),
            back_overhang: format!("{:#}", r.back_overhang),
        }),
        floor: project.floor.as_ref().map(|f| Floor {
            joists: f.joist.to_string(),
            beams: format!("{} {}", f.beams, f.beam),
        }),
        porch: project.porch.as_ref().map(|p| Porch {
            depth: p.depth.to_string(),
            deck: p.deck.is_some(),
        }),
        framing: Framing {
            studs: project.rules.wall_lumber.to_string(),
            spacing: format!("{:#} on center", project.rules.on_center),
        },
        walls,
        openings,
    }
}

fn wall<'a>(
    id: &str,
    length: Option<String>,
    openings: impl IntoIterator<Item = &'a sticks_core::Opening>,
) -> Wall {
    Wall {
        id: id.to_string(),
        length,
        openings: openings
            .into_iter()
            .enumerate()
            .map(|(i, o)| Opening {
                id: format!("{id}-{}", i + 1),
                kind: kind_name(o.kind),
                width: o.width.to_string(),
                height: o.height.to_string(),
                from_left_end: o.offset.to_string(),
                above_floor: (o.kind != OpeningKind::Door).then(|| o.sill_height.to_string()),
            })
            .collect(),
    }
}
