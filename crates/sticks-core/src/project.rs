//! The project file: a plain-text TOML description of a structure that diffs
//! cleanly in Git.

use std::fmt;
use std::str::FromStr;

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
    /// Sheet goods on offer (subfloor, later sheathing).
    #[serde(default, rename = "sheet", skip_serializing_if = "Vec::is_empty")]
    pub sheets: Vec<SheetStock>,
    /// The footprint of a rectangular building. When present, its four outside
    /// walls (`front`, `back`, `left`, `right`) are generated from it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub building: Option<Building>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floor: Option<Floor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roof: Option<Roof>,
    #[serde(default, rename = "wall", skip_serializing_if = "Vec::is_empty")]
    pub walls: Vec<Wall>,
}

/// A rectangular footprint. The front wall runs along `width`; the side walls
/// run along `depth`, front to back.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Building {
    /// Outside to outside, along the front wall.
    pub width: Length,
    /// Outside to outside, front to back.
    pub depth: Length,
    /// The front wall, subfloor to top of cap plate. With a lean-to roof this
    /// is the high wall, and the back and side walls follow the pitch.
    pub height: Length,
}

/// A floor platform on beams, covering the building's footprint. Joists run
/// front to back; beams and rim joists run along the front.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Floor {
    /// Joist and rim size. Default 2x6.
    pub joist: Nominal,
    /// Joist spacing, on center. Defaults to the rules' spacing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_center: Option<Length>,
    /// Beam size. Default 4x6.
    pub beam: Nominal,
    /// Boards laminated into each beam, such as 2 for a doubled 2x8. Default 1.
    pub beam_plies: u8,
    /// Beam lines, front to back, the outer two under the rims. Default 2.
    pub beams: u8,
    /// What the beams sit on. Default piers.
    pub support: FloorSupport,
    /// Most room between piers along a beam. Default 6'.
    pub pier_spacing: Length,
    /// From each end of a beam to the center of its end pier. Default 12".
    pub pier_inset: Length,
    /// Subfloor sheet thickness. Default 3/4".
    pub subfloor: Length,
}

impl Default for Floor {
    fn default() -> Self {
        Floor {
            joist: Nominal::new(2, 6),
            on_center: None,
            beam: Nominal::new(4, 6),
            beam_plies: 1,
            beams: 2,
            support: FloorSupport::Piers,
            pier_spacing: Length::feet(6),
            pier_inset: Length::inches(12),
            subfloor: Length::from_sixteenths(12),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FloorSupport {
    /// Beams on piers; beam joints land on a pier.
    #[default]
    Piers,
    /// Beams laid on grade as skids; no piers.
    Skids,
}

/// The roof over a building.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Roof {
    #[serde(default)]
    pub kind: RoofKind,
    /// Rise per foot of run, written like `3:12`.
    pub pitch: Pitch,
    /// Rafter size. Default 2x6.
    #[serde(default = "default_rafter")]
    pub rafter: Nominal,
    /// Rafter spacing, on center. Defaults to the rules' spacing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_center: Option<Length>,
    /// Level distance the rafters run past the front wall. Default 12".
    #[serde(default = "default_overhang")]
    pub front_overhang: Length,
    /// Level distance the rafters run past the back wall. Default 12".
    #[serde(default = "default_overhang")]
    pub back_overhang: Length,
    /// Fascia size, nailed to the rafter tails. Defaults to the rafter size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fascia: Option<Nominal>,
}

fn default_rafter() -> Nominal {
    Nominal::new(2, 6)
}

fn default_overhang() -> Length {
    Length::inches(12)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoofKind {
    /// A single slope, high at the front wall and low at the back.
    #[default]
    LeanTo,
}

/// A roof pitch: inches of rise per 12" of level run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Pitch {
    pub rise: Length,
}

impl Pitch {
    /// The run every pitch is written against.
    pub const RUN: Length = Length::inches(12);

    pub fn new(rise: Length) -> Self {
        Pitch { rise }
    }

    /// How far the roof rises over `run`, to the nearest sixteenth.
    pub fn rise_over(self, run: Length) -> Length {
        run.mul_div(self.rise.sixteenths(), Self::RUN.sixteenths())
    }

    /// Length along the slope for a level `run`, to the nearest sixteenth.
    pub fn slope_length(self, run: Length) -> Length {
        Length::from_inches_f64(run.as_inches_f64() * self.secant())
    }

    /// Length along the slope per unit of level run.
    pub fn secant(self) -> f64 {
        let r = self.rise.as_inches_f64() / 12.0;
        (1.0 + r * r).sqrt()
    }
}

impl fmt::Display for Pitch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rise = format!("{:#}", self.rise);
        write!(f, "{}:12", rise.trim_end_matches('"'))
    }
}

impl FromStr for Pitch {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bad = || format!("{s:?} is not a pitch; write rise per foot like 3:12");
        let (rise, run) = s.split_once(':').ok_or_else(bad)?;
        if run.trim() != "12" {
            return Err(bad());
        }
        let rise: Length = rise.trim().parse().map_err(|_| bad())?;
        if rise <= Length::ZERO || rise >= Length::inches(24) {
            return Err(format!(
                "{s:?} is outside what a lean-to can use; pick a rise between 0 and 24"
            ));
        }
        Ok(Pitch { rise })
    }
}

impl TryFrom<String> for Pitch {
    type Error = String;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<Pitch> for String {
    fn from(p: Pitch) -> String {
        p.to_string()
    }
}

/// Sheet goods on offer in one thickness.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SheetStock {
    pub thickness: Length,
    /// Long side. Default 8'.
    #[serde(default = "default_sheet_length")]
    pub length: Length,
    /// Short side. Default 4'.
    #[serde(default = "default_sheet_width")]
    pub width: Length,
    /// Price per sheet in cents. Optional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_cents: Option<u32>,
}

fn default_sheet_length() -> Length {
    Length::feet(8)
}

fn default_sheet_width() -> Length {
    Length::feet(4)
}

impl SheetStock {
    /// A plain 4x8 sheet with no price, for thicknesses the project lists no
    /// stock for.
    pub fn standard(thickness: Length) -> Self {
        SheetStock {
            thickness,
            length: default_sheet_length(),
            width: default_sheet_width(),
            price_cents: None,
        }
    }
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
///
/// A wall whose id names a side of the building (`front`, `back`, `left`,
/// `right`) takes its length, height and ends from the building and leaves
/// those fields out. Any other wall gives them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wall {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length: Option<Length>,
    /// Overall height, bottom of bottom plate to top of top plates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<Length>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left_end: Option<WallEnd>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right_end: Option<WallEnd>,
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
        assert_eq!(p.walls[0].left_end, Some(WallEnd::Corner));
        assert_eq!(p.walls[0].right_end, None);
        assert_eq!(p.walls[0].tees, vec![Length::feet(11)]);
    }

    #[test]
    fn round_trips() {
        let p = Project::from_toml(SHED_WALL).unwrap();
        let text = p.to_toml().unwrap();
        assert_eq!(Project::from_toml(&text).unwrap(), p);
    }

    #[test]
    fn loads_a_building_with_floor_and_roof() {
        let p = Project::from_toml(
            r#"
name = "Shed"
[building]
width = "18'"
depth = "14'"
height = "10'"
[floor]
beams = 3
[roof]
pitch = "3-1/2:12"
[[sheet]]
thickness = "3/4\""
price_cents = 4599
[[wall]]
id = "front"
"#,
        )
        .unwrap();
        let roof = p.roof.as_ref().unwrap();
        assert_eq!(roof.pitch.rise, Length::from_sixteenths(56));
        assert_eq!(roof.pitch.to_string(), "3-1/2:12");
        assert_eq!(roof.rafter, Nominal::new(2, 6));
        assert_eq!(roof.front_overhang, Length::inches(12));
        let floor = p.floor.as_ref().unwrap();
        assert_eq!(floor.beams, 3);
        assert_eq!(floor.joist, Nominal::new(2, 6));
        assert_eq!(p.sheets[0].length, Length::feet(8));
        assert_eq!(p.walls[0].length, None);
        let text = p.to_toml().unwrap();
        assert_eq!(Project::from_toml(&text).unwrap(), p);
    }

    #[test]
    fn pitch_rejects_other_runs() {
        assert!("3:10".parse::<Pitch>().is_err());
        assert!("0:12".parse::<Pitch>().is_err());
        assert_eq!(
            Pitch::new(Length::inches(3))
                .rise_over(Length::inches(164) + Length::from_sixteenths(8)),
            Length::from_sixteenths(41 * 16 + 2)
        );
    }

    #[test]
    fn unknown_keys_are_errors() {
        let err = Project::from_toml("name = \"x\"\nwals = []").unwrap_err();
        assert!(err.to_string().contains("wals"));
    }
}
