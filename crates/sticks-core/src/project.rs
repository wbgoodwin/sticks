//! The project file: a plain-text TOML description of a structure that diffs
//! cleanly in Git.

use serde::{Deserialize, Serialize};

use crate::lumber::Nominal;
use crate::units::Length;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub name: String,
    #[serde(default)]
    pub rules: Rules,
    #[serde(default, rename = "stock", skip_serializing_if = "Vec::is_empty")]
    pub stock: Vec<Stock>,
    #[serde(default, rename = "wall", skip_serializing_if = "Vec::is_empty")]
    pub walls: Vec<Wall>,
}

/// Framing conventions. Every default is documented here so builders can see
/// what the tool assumes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Rules {
    /// Stud and joist spacing, on center. Default 16".
    pub on_center: Length,
    /// Saw blade width lost on every cut. Default 1/8".
    pub kerf: Length,
    /// Trimmed off each stock board to square factory ends. Default 0".
    pub end_trim: Length,
    /// Lumber size for wall studs and plates. Default 2x4.
    pub wall_lumber: Nominal,
    /// Which end of each wall stud layout is measured from. Default left.
    pub layout_from: LayoutStart,
    /// How a wall end marked `corner` is backed. Default three-stud.
    pub corner_style: CornerStyle,
}

impl Default for Rules {
    fn default() -> Self {
        Rules {
            on_center: Length::inches(16),
            kerf: Length::from_sixteenths(2),
            end_trim: Length::ZERO,
            wall_lumber: Nominal::new(2, 4),
            layout_from: LayoutStart::Left,
            corner_style: CornerStyle::ThreeStud,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutStart {
    #[default]
    Left,
    Right,
}

/// Corner backing on the wall that runs through a corner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CornerStyle {
    /// End stud, 1-1/2" spacer blocks, then a backer stud. The butting wall's
    /// end stud makes the third.
    #[default]
    ThreeStud,
}

/// What happens at one end of a wall.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WallEnd {
    /// A single end stud: a free end, or a wall that butts into another.
    #[default]
    Plain,
    /// This wall runs through a corner and backs the wall that butts into it.
    Corner,
}

impl WallEnd {
    fn is_plain(&self) -> bool {
        *self == WallEnd::Plain
    }
}

/// Stock boards available at the lumber yard for one nominal size.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stock {
    pub size: Nominal,
    pub lengths: Vec<Length>,
    /// Price per board in cents, parallel to `lengths`. Optional.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prices_cents: Vec<u32>,
}

/// A straight stud wall, seen from outside.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wall {
    pub id: String,
    pub length: Length,
    /// Overall height, bottom of bottom plate to top of top plates.
    pub height: Length,
    #[serde(default, skip_serializing_if = "WallEnd::is_plain")]
    pub left_end: WallEnd,
    #[serde(default, skip_serializing_if = "WallEnd::is_plain")]
    pub right_end: WallEnd,
    /// Centers of partitions that tee into this wall, from its left end.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tees: Vec<Length>,
    #[serde(default, rename = "opening", skip_serializing_if = "Vec::is_empty")]
    pub openings: Vec<Opening>,
}

/// A door, window or rough opening, positioned from the wall's left end.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Opening {
    pub kind: OpeningKind,
    /// Distance from the wall's left end to the left side of the rough opening.
    pub offset: Length,
    pub width: Length,
    pub height: Length,
    /// Bottom of the rough opening above the floor. Doors use 0.
    #[serde(default)]
    pub sill_height: Length,
    /// Header size, chosen by the user (the tool does not size headers).
    pub header: Nominal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpeningKind {
    Door,
    Window,
    /// A small penetration, such as a mini-split line set.
    Penetration,
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error(transparent)]
    Parse(#[from] toml::de::Error),
    #[error(transparent)]
    Write(#[from] toml::ser::Error),
}

impl Project {
    pub fn from_toml(text: &str) -> Result<Self, ProjectError> {
        Ok(toml::from_str(text)?)
    }

    pub fn to_toml(&self) -> Result<String, ProjectError> {
        Ok(toml::to_string_pretty(self)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHED_WALL: &str = r#"
name = "Backyard shed"

[rules]
on_center = "16\""
kerf = "1/8\""

[[stock]]
size = "2x4"
lengths = ["8'", "10'", "12'", "16'"]
prices_cents = [498, 698, 849, 1149]

[[wall]]
id = "W1"
length = "18'"
height = "10'"
left_end = "corner"
tees = ["11'"]

[[wall.opening]]
kind = "door"
offset = "6' 3\""
width = "38-1/2\""
height = "82-1/2\""
header = "2x8"

[[wall.opening]]
kind = "penetration"
offset = "14'"
width = "4\""
height = "4\""
sill_height = "1'"
header = "2x4"
"#;

    #[test]
    fn loads_a_project_file() {
        let p = Project::from_toml(SHED_WALL).unwrap();
        assert_eq!(p.rules.kerf, Length::from_sixteenths(2));
        assert_eq!(p.rules.wall_lumber, Nominal::new(2, 4));
        assert_eq!(p.walls[0].openings[0].width.to_string(), "3' 2-1/2\"");
        assert_eq!(p.walls[0].openings[1].kind, OpeningKind::Penetration);
        assert_eq!(p.stock[0].lengths[3], Length::feet(16));
        assert_eq!(p.walls[0].left_end, WallEnd::Corner);
        assert_eq!(p.walls[0].right_end, WallEnd::Plain);
        assert_eq!(p.walls[0].tees, vec![Length::feet(11)]);
    }

    #[test]
    fn round_trips() {
        let p = Project::from_toml(SHED_WALL).unwrap();
        let text = p.to_toml().unwrap();
        assert_eq!(Project::from_toml(&text).unwrap(), p);
    }

    #[test]
    fn unknown_keys_are_errors() {
        let err = Project::from_toml("name = \"x\"\nwals = []").unwrap_err();
        assert!(err.to_string().contains("wals"));
    }
}
