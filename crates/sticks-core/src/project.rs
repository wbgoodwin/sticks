//! The project file: a plain-text TOML description of a structure that diffs
//! cleanly in Git.

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::joint::JointKind;
use crate::lumber::Nominal;
use crate::units::Length;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub name: String,
    /// Stages of the build the builder already has the lumber for, such as
    /// `floor` or `wall front`, kept off the shopping list. See the rules
    /// crate's `Section` for the names.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_hand: Vec<String>,
    #[serde(default)]
    pub rules: Rules,
    #[serde(default, rename = "stock", skip_serializing_if = "Vec::is_empty")]
    pub stock: Vec<Stock>,
    /// Sheet goods on offer (subfloor and sheathing).
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub porch: Option<Porch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sheathing: Option<Sheathing>,
    /// Connectors and their nails. Left out, the shopping list carries the
    /// connectors every design needs, nailed, with no prices.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hardware: Option<Hardware>,
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
    /// is the high wall, and the back and side walls follow the pitch. Under
    /// a gable roof every wall is this tall and the gables sit on the sides.
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
    /// Post size, when the beams sit on posts. Default 4x4.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post: Option<Nominal>,
    /// Post length, pier top to beam bottom, when the beams sit on posts.
    /// Default 12".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_height: Option<Length>,
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
            post: None,
            post_height: None,
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
    /// Beams on short posts, each post on a pier; beam joints land on a post.
    Posts,
}

impl Floor {
    /// The post under each beam, when the beams sit on posts.
    pub fn post_size(&self) -> Nominal {
        self.post.unwrap_or(Nominal::new(4, 4))
    }

    /// Pier top to beam bottom, when the beams sit on posts.
    pub fn post_length(&self) -> Length {
        self.post_height.unwrap_or(Length::inches(12))
    }
}

/// The metal connectors at the joints and the nails or screws that hold
/// them. Every connector the design needs is on the list unless `parts`
/// says `none` for its joint; the extra ties are off unless asked for.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Hardware {
    /// Nails or connector screws. Default nails.
    pub fasten_with: Fastening,
    /// Every extra tie for a high-wind area: rafters at the ridge, joists on
    /// beams, studs to plates and the deck's lateral ties.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub high_wind: bool,
    /// Extra ties to add without `high_wind`, by joint, such as
    /// `joist_on_beam`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub add: Vec<JointKind>,
    /// A model to use in place of the catalog's pick for a kind of joint, or
    /// `none` to leave that joint's connector off, such as
    /// `rafter_on_plate = "H1"`.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub parts: BTreeMap<JointKind, String>,
    /// Price in cents by model for connectors, and per box for fasteners,
    /// such as `"H2.5A" = 89`.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub prices_cents: BTreeMap<String, u32>,
}

impl Hardware {
    /// Whether joints of `kind` get a connector.
    pub fn wants(&self, kind: JointKind) -> bool {
        if self.parts.get(&kind).is_some_and(|p| p == "none") {
            return false;
        }
        kind.required() || self.high_wind || self.add.contains(&kind)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fastening {
    /// The nails each connector's catalog listing calls for.
    #[default]
    Nails,
    /// Connector screws, one in each nail hole.
    Screws,
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
    /// Fascia size, nailed to the rafter tails. Defaults to two sizes up
    /// from the rafter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fascia: Option<Nominal>,
    /// Gable roofs: the ridge board. Defaults to two sizes up from the rafter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ridge: Option<Nominal>,
    /// Gable roofs: collar ties across rafter pairs, in the upper third.
    /// Default 2x4.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collar_tie: Option<Nominal>,
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
    /// Two slopes meeting at a ridge that runs along the front, with the
    /// gables over the side walls.
    Gable,
}

/// A covered porch across the front of the building: posts on piers, a beam
/// on the posts, and a roof hung from a ledger on the front wall that slopes
/// down to the beam.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Porch {
    /// From the front wall's outside face to the outside of the posts.
    /// Default 6'.
    pub depth: Length,
    /// Post size. Default 4x4.
    pub post: Nominal,
    /// Most room between posts, center to center. Default 8'.
    pub post_spacing: Length,
    /// Beam size. Default 2x8.
    pub beam: Nominal,
    /// Boards laminated into the beam. Default 2.
    pub beam_plies: u8,
    /// Rafter size. Defaults to the roof's rafters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rafter: Option<Nominal>,
    /// Defaults to the roof's pitch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pitch: Option<Pitch>,
    /// Level distance the rafters run past the beam. Default 12".
    pub overhang: Length,
    /// Ledger size. Defaults to two sizes up from the rafter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ledger: Option<Nominal>,
    /// Top of the ledger above the subfloor. Defaults to just under the front
    /// wall's top plates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ledger_height: Option<Length>,
    /// A deck under the porch roof. Leave it out for open ground.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deck: Option<Deck>,
}

/// A porch deck a step below the subfloor: joists hung from a ledger on the
/// floor's front rim, out to a rim over a beam on piers under the porch
/// posts, with decking boards across the joists. The posts stand on it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Deck {
    /// Joist, rim and ledger size. Defaults to the floor's joists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub joist: Option<Nominal>,
    /// Beam size, under the outer rim. Default 4x6.
    pub beam: Nominal,
    /// Decking boards, laid along the front. Default 5/4x6.
    pub decking: Nominal,
    /// Space between decking boards. Default 1/8".
    pub gap: Length,
    /// How far the top of the decking sits below the subfloor. Default 1".
    pub step_down: Length,
}

impl Default for Deck {
    fn default() -> Self {
        Deck {
            joist: None,
            beam: Nominal::new(4, 6),
            decking: "5/4x6".parse().expect("5/4x6 is a lumber size"),
            gap: Length::from_sixteenths(2),
            step_down: Length::inches(1),
        }
    }
}

impl Default for Porch {
    fn default() -> Self {
        Porch {
            depth: Length::feet(6),
            post: Nominal::new(4, 4),
            post_spacing: Length::feet(8),
            beam: Nominal::new(2, 8),
            beam_plies: 2,
            rafter: None,
            pitch: None,
            overhang: Length::inches(12),
            ledger: None,
            ledger_height: None,
            deck: None,
        }
    }
}

/// Sheathing on the outside walls and the roof. Leave a thickness out to skip
/// that part.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Sheathing {
    /// Wall sheathing thickness, such as 7/16".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wall: Option<Length>,
    /// Roof sheathing thickness, such as 1/2". Also covers the porch roof.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roof: Option<Length>,
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
                "{s:?} is outside what Sticks can frame; pick a rise between 0 and 24"
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
    fn loads_hardware_and_posts() {
        let p = Project::from_toml(
            r#"
name = "Shed"
[building]
width = "12'"
depth = "8'"
height = "8'"
[floor]
support = "posts"
post_height = "18\""
[hardware]
fasten_with = "screws"
add = ["joist_on_beam"]
[hardware.parts]
rafter_on_plate = "H1"
stud_to_plate = "none"
[hardware.prices_cents]
"H2.5A" = 89
"#,
        )
        .unwrap();
        let floor = p.floor.as_ref().unwrap();
        assert_eq!(floor.support, FloorSupport::Posts);
        assert_eq!(floor.post_size(), Nominal::new(4, 4));
        assert_eq!(floor.post_length(), Length::inches(18));
        let hw = p.hardware.as_ref().unwrap();
        assert_eq!(hw.fasten_with, Fastening::Screws);
        assert!(hw.wants(JointKind::JoistOnBeam));
        assert!(hw.wants(JointKind::RafterOnPlate));
        assert!(!hw.wants(JointKind::StudToPlate));
        assert!(!hw.wants(JointKind::DeckLateral));
        assert_eq!(hw.parts[&JointKind::RafterOnPlate], "H1");
        assert_eq!(hw.prices_cents["H2.5A"], 89);
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
