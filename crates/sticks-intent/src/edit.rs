//! Typed design changes. Every value in an edit is already resolved against
//! the project ("2' wider" becomes the new width), so an edit can be shown,
//! checked and applied without reading the request again.

use std::fmt;

use serde::{Deserialize, Serialize};
use sticks_core::{
    Building, Length, Nominal, Opening, OpeningKind, Pitch, Porch, PorchDeck, Project, RoofKind,
    Wall,
};

/// A side of the building, named as you'd stand outside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Front,
    Back,
    Left,
    Right,
}

impl Side {
    pub const ALL: [Side; 4] = [Side::Front, Side::Back, Side::Left, Side::Right];

    /// The wall id the project file uses for this side.
    pub fn id(self) -> &'static str {
        match self {
            Side::Front => "front",
            Side::Back => "back",
            Side::Left => "left",
            Side::Right => "right",
        }
    }

    /// How long this side's wall is, outside to outside.
    pub fn length(self, b: &Building) -> Length {
        match self {
            Side::Front | Side::Back => b.width,
            Side::Left | Side::Right => b.depth,
        }
    }
}

/// One of the building's three overall sizes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dimension {
    /// Along the front wall.
    Width,
    /// Front to back.
    Depth,
    /// The front wall's height.
    Height,
}

impl Dimension {
    pub const ALL: [Dimension; 3] = [Dimension::Width, Dimension::Depth, Dimension::Height];

    pub fn label(self) -> &'static str {
        match self {
            Dimension::Width => "width",
            Dimension::Depth => "depth",
            Dimension::Height => "height",
        }
    }

    pub fn of(self, b: &Building) -> Length {
        match self {
            Dimension::Width => b.width,
            Dimension::Depth => b.depth,
            Dimension::Height => b.height,
        }
    }
}

/// Which framing members a lumber change applies to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Framing {
    Studs,
    Joists,
    Rafters,
}

impl Framing {
    pub const ALL: [Framing; 3] = [Framing::Studs, Framing::Joists, Framing::Rafters];

    pub fn label(self) -> &'static str {
        match self {
            Framing::Studs => "studs",
            Framing::Joists => "joists",
            Framing::Rafters => "rafters",
        }
    }
}

/// One change to the design.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Edit {
    Resize {
        dimension: Dimension,
        to: Length,
    },
    SetPitch {
        pitch: Pitch,
    },
    SetRoofKind {
        kind: RoofKind,
    },
    /// Both the front and back overhangs.
    SetOverhang {
        to: Length,
    },
    /// Adds a porch, or changes the depth of the one there.
    SetPorch {
        depth: Length,
    },
    RemovePorch,
    AddDeck,
    RemoveDeck,
    AddOpening {
        wall: Side,
        kind: OpeningKind,
        /// From the wall's left end, seen from outside.
        offset: Length,
        width: Length,
        height: Length,
        sill_height: Length,
        header: Nominal,
    },
    /// Resizes or moves the opening at `index` in that wall's list.
    ChangeOpening {
        wall: Side,
        index: usize,
        /// What it is, for the preview; the change keeps it.
        kind: OpeningKind,
        offset: Length,
        width: Length,
        height: Length,
        header: Nominal,
    },
    /// The opening at `index` in that wall's list.
    RemoveOpening {
        wall: Side,
        index: usize,
    },
    SetSpacing {
        on_center: Length,
    },
    SetLumber {
        framing: Framing,
        size: Nominal,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EditError {
    #[error("this project has no building to {0}")]
    NoBuilding(&'static str),
    #[error("this project has no roof")]
    NoRoof,
    #[error("there's no porch to {0}")]
    NoPorch(&'static str),
    #[error("the {0} wall has no opening {1}")]
    NoOpening(&'static str, usize),
}

impl Edit {
    pub fn apply(&self, p: &mut Project) -> Result<(), EditError> {
        match *self {
            Edit::Resize { dimension, to } => {
                let b = p.building.as_mut().ok_or(EditError::NoBuilding("resize"))?;
                match dimension {
                    Dimension::Width => b.width = to,
                    Dimension::Depth => b.depth = to,
                    Dimension::Height => b.height = to,
                }
            }
            Edit::SetPitch { pitch } => p.roof.as_mut().ok_or(EditError::NoRoof)?.pitch = pitch,
            Edit::SetRoofKind { kind } => p.roof.as_mut().ok_or(EditError::NoRoof)?.kind = kind,
            Edit::SetOverhang { to } => {
                let r = p.roof.as_mut().ok_or(EditError::NoRoof)?;
                r.front_overhang = to;
                r.back_overhang = to;
            }
            Edit::SetPorch { depth } => {
                if p.building.is_none() {
                    return Err(EditError::NoBuilding("put a porch on"));
                }
                p.porch.get_or_insert_with(Porch::default).depth = depth;
            }
            Edit::RemovePorch => {
                p.porch.take().ok_or(EditError::NoPorch("remove"))?;
            }
            Edit::AddDeck => {
                let porch = p
                    .porch
                    .as_mut()
                    .ok_or(EditError::NoPorch("put a deck under"))?;
                porch.deck.get_or_insert_with(PorchDeck::default);
            }
            Edit::RemoveDeck => {
                if let Some(porch) = p.porch.as_mut() {
                    porch.deck = None;
                }
            }
            Edit::AddOpening {
                wall,
                kind,
                offset,
                width,
                height,
                sill_height,
                header,
            } => wall_mut(p, wall).openings.push(Opening {
                kind,
                offset,
                width,
                height,
                sill_height,
                header,
            }),
            Edit::ChangeOpening {
                wall,
                index,
                offset,
                width,
                height,
                header,
                ..
            } => {
                let Some(o) = wall_mut(p, wall).openings.get_mut(index) else {
                    return Err(EditError::NoOpening(wall.id(), index));
                };
                o.offset = offset;
                o.width = width;
                o.height = height;
                o.header = header;
            }
            Edit::RemoveOpening { wall, index } => {
                let openings = &mut wall_mut(p, wall).openings;
                if index >= openings.len() {
                    return Err(EditError::NoOpening(wall.id(), index));
                }
                openings.remove(index);
            }
            Edit::SetSpacing { on_center } => {
                p.rules.on_center = on_center;
                if let Some(f) = p.floor.as_mut() {
                    f.on_center = None;
                }
                if let Some(r) = p.roof.as_mut() {
                    r.on_center = None;
                }
            }
            Edit::SetLumber { framing, size } => match framing {
                Framing::Studs => p.rules.wall_lumber = size,
                Framing::Joists => p.floor.get_or_insert_with(Default::default).joist = size,
                Framing::Rafters => p.roof.as_mut().ok_or(EditError::NoRoof)?.rafter = size,
            },
        }
        Ok(())
    }
}

/// The project's entry for a building wall, added if the file doesn't list it.
fn wall_mut(p: &mut Project, side: Side) -> &mut Wall {
    let i = match p.walls.iter().position(|w| w.id == side.id()) {
        Some(i) => i,
        None => {
            p.walls.push(Wall {
                id: side.id().to_string(),
                length: None,
                height: None,
                left_end: None,
                right_end: None,
                tees: Vec::new(),
                openings: Vec::new(),
            });
            p.walls.len() - 1
        }
    };
    &mut p.walls[i]
}

pub fn kind_name(kind: OpeningKind) -> &'static str {
    match kind {
        OpeningKind::Door => "door",
        OpeningKind::Window => "window",
        OpeningKind::Penetration => "penetration",
    }
}

/// Builder English, for the preview: "Make the building 20' wide".
impl fmt::Display for Edit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Edit::Resize { dimension, to } => {
                let word = match dimension {
                    Dimension::Width => "wide",
                    Dimension::Depth => "deep",
                    Dimension::Height => "tall at the front",
                };
                write!(f, "Make the building {to} {word}")
            }
            Edit::SetPitch { pitch } => write!(f, "Set the roof pitch to {pitch}"),
            Edit::SetRoofKind { kind } => match kind {
                RoofKind::LeanTo => write!(f, "Make the roof a lean-to"),
                RoofKind::Gable => write!(f, "Make the roof a gable"),
            },
            Edit::SetOverhang { to } => write!(f, "Set the front and back overhangs to {to:#}"),
            Edit::SetPorch { depth } => write!(f, "Make the porch {depth} deep"),
            Edit::RemovePorch => write!(f, "Remove the porch"),
            Edit::AddDeck => write!(f, "Put a deck under the porch"),
            Edit::RemoveDeck => write!(f, "Take the deck out from under the porch"),
            Edit::AddOpening {
                wall,
                kind,
                offset,
                width,
                height,
                ..
            } => write!(
                f,
                "Add a {width} x {height} {} to the {} wall, {offset} from its left end",
                kind_name(*kind),
                wall.id()
            ),
            Edit::ChangeOpening {
                wall,
                kind,
                offset,
                width,
                height,
                ..
            } => write!(
                f,
                "Make the {} wall's {} {width} x {height}, {offset} from its left end",
                wall.id(),
                kind_name(*kind)
            ),
            Edit::RemoveOpening { wall, index } => {
                write!(
                    f,
                    "Remove opening {} from the {} wall",
                    index + 1,
                    wall.id()
                )
            }
            Edit::SetSpacing { on_center } => {
                write!(f, "Space studs, joists and rafters {on_center:#} on center")
            }
            Edit::SetLumber { framing, size } => write!(f, "Use {size} {}", framing.label()),
        }
    }
}
