//! The schema's types. Their doc comments become the JSON Schema's
//! descriptions, which is what a model reads, so they are written as help
//! for a builder.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sticks_core::{
    BeamStyle, Deck, DeckCode, DeckSupport, DeckingMaterial, Floor, FloorSupport, FramingSpecies,
    LayoutStart, Length, Nominal, OpeningKind, Pitch, Porch, PorchDeck, Roof, RoofKind, Rules,
    Sheathing, Species, WallEnd,
};

use crate::spec::spec;

/// A design Sticks can frame and check for a permit: what gets built, and
/// the few framing conventions that apply to all of it. Say only what the
/// person asked for; anything left out takes Sticks' documented default and
/// is shown to them as an assumption.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(title = "Sticks design")]
pub struct Design {
    /// The schema version. Always "sticks/1".
    pub schema: Version,
    /// What the person calls the project, like "Backyard shed".
    pub name: String,
    /// Framing conventions for every structure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub framing: Option<FramingSpec>,
    /// What gets built. Sticks frames at most one building, one porch on
    /// that building, and one deck, plus any number of freestanding walls.
    pub structures: Vec<Structure>,
    /// Questions for the person about things the request left out that
    /// change the permit or the structure, such as how tall it is or what it
    /// stands on. Leave it empty when a default will do.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub questions: Vec<String>,
}

/// The schema version a design is written against.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Version {
    #[default]
    #[serde(rename = "sticks/1")]
    V1,
}

spec! {
    /// Framing conventions shared by every structure.
    pub struct FramingSpec => Rules {
        extra {}
        /// Stud, joist and rafter spacing, on center. Default 16".
        fill on_center: Length,
        /// Lumber size for wall studs and plates. Default 2x4.
        fill wall_lumber: Nominal,
        /// Which end of each wall the stud layout is measured from, seen
        /// from outside. Default left.
        fill layout_from: LayoutStart,
    }
}

/// One thing to build. `kind` says which, and picks the fields that follow.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Structure {
    /// A rectangular building, like a shed: four outside stud walls, on a
    /// floor platform or not, under a roof or not.
    Building(BuildingSpec),
    /// A covered porch across the front of a building: posts on piers, a
    /// beam on the posts, and a roof hung from a ledger on the front wall.
    /// It can have a deck under it.
    Porch(PorchSpec),
    /// A deck standing on its own footings, away from any building.
    Deck(DeckSpec),
    /// A straight stud wall standing on its own, like a partition or a
    /// garden wall.
    Wall(WallSpec),
}

impl Structure {
    pub fn id(&self) -> &str {
        match self {
            Structure::Building(b) => &b.id,
            Structure::Porch(p) => &p.id,
            Structure::Deck(d) => &d.id,
            Structure::Wall(w) => &w.id,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Structure::Building(_) => "building",
            Structure::Porch(_) => "porch",
            Structure::Deck(_) => "deck",
            Structure::Wall(_) => "wall",
        }
    }
}

/// A rectangular building. The front wall runs along `width`; the side
/// walls run along `depth`, front to back.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BuildingSpec {
    /// A short name other structures use to refer to it, like "shed".
    pub id: String,
    /// Outside to outside, along the front wall.
    pub width: Length,
    /// Outside to outside, front to back.
    pub depth: Length,
    /// The front wall, subfloor to top of plates. Under a lean-to roof this
    /// is the high wall and the others follow the pitch down to the back;
    /// under a gable roof every wall is this tall.
    pub height: Length,
    /// The floor platform the walls stand on. Leave it out when the walls
    /// stand on a slab.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floor: Option<FloorSpec>,
    /// The roof. Leave it out for walls only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roof: Option<RoofSpec>,
    /// Sheathing on the outside walls and roof. Leave it out, or leave out a
    /// thickness, to skip that part.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sheathing: Option<Sheathing>,
    /// Doors, windows and tees on the four outside walls. List a wall only
    /// when something goes in it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub walls: Vec<SideWall>,
}

/// One of a building's four outside walls and what goes in it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SideWall {
    pub side: Side,
    /// Doors, windows and penetrations in this wall.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub openings: Vec<OpeningSpec>,
    /// Centers of inside partitions that tee into this wall, measured from
    /// its left end seen from outside.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tees: Vec<Length>,
}

/// A side of a building. Front and back run along its width; left and right,
/// seen from outside the front, run along its depth.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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

    pub fn from_id(id: &str) -> Option<Side> {
        Side::ALL.into_iter().find(|s| s.id() == id)
    }
}

/// A door, window or small penetration in a wall. Sizes are rough openings,
/// the framed hole.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OpeningSpec {
    pub kind: OpeningKind,
    /// Where it goes along the wall.
    pub place: Place,
    /// Rough opening width. Default 38-1/2" for a door (a 36" door slab),
    /// 36" for a window, 4" for a penetration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<Length>,
    /// Rough opening height. Default 82-1/2" for a door, 36" for a window,
    /// 4" for a penetration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<Length>,
    /// Bottom of the rough opening above the floor. Default 0" for a door,
    /// 44" for a window, 1' for a penetration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sill_height: Option<Length>,
    /// Header size over the opening. Default 2x8 for a door, 2x6 for a
    /// window, 2x4 for a penetration. Sticks doesn't size headers yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header: Option<Nominal>,
}

/// Where an opening sits along its wall, seen from outside.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "from", rename_all = "snake_case", deny_unknown_fields)]
pub enum Place {
    /// Its left side `distance` from the wall's left end.
    Left { distance: Length },
    /// Its right side `distance` from the wall's right end.
    Right { distance: Length },
    /// Centered along the wall.
    Center,
}

spec! {
    /// A floor platform on beams, covering the building's footprint. Joists
    /// run front to back; beams and rim joists run along the front.
    pub struct FloorSpec => Floor {
        extra {}
        /// Joist and rim size. Default 2x6.
        fill joist: Nominal,
        /// Joist spacing, on center. Defaults to the framing spacing.
        keep on_center: Length,
        /// Beam size. Default 4x6.
        fill beam: Nominal,
        /// Boards laminated into each beam, such as 2 for a doubled 2x8.
        /// Default 1.
        fill beam_plies: u8,
        /// Beam lines, front to back, the outer two under the rims.
        /// Default 2.
        fill beams: u8,
        /// What the beams sit on. Default piers.
        fill support: FloorSupport,
        /// Post size, when the beams sit on posts. Default 4x4.
        keep post: Nominal,
        /// Post length, pier top to beam bottom, when the beams sit on
        /// posts. Default 12".
        keep post_height: Length,
        /// Most room between piers along a beam. Default 6'.
        fill pier_spacing: Length,
        /// From each end of a beam to the center of its end pier.
        /// Default 12".
        fill pier_inset: Length,
        /// Subfloor sheet thickness. Default 3/4".
        fill subfloor: Length,
    }
}

spec! {
    /// The roof over a building.
    pub struct RoofSpec => Roof {
        extra {}
        /// Lean-to (one slope, high at the front) or gable (a ridge along
        /// the front, gables over the side walls). Default lean-to.
        fill kind: RoofKind,
        /// Default 4:12.
        fill pitch: Pitch,
        /// Rafter size. Default 2x6.
        fill rafter: Nominal,
        /// Rafter spacing, on center. Defaults to the framing spacing.
        keep on_center: Length,
        /// Level distance the rafters run past the front wall. Default 12".
        fill front_overhang: Length,
        /// Level distance the rafters run past the back wall. Default 12".
        fill back_overhang: Length,
        /// Fascia size. Defaults to two sizes up from the rafter.
        keep fascia: Nominal,
        /// Gable roofs: the ridge board. Defaults to two sizes up from the
        /// rafter.
        keep ridge: Nominal,
        /// Gable roofs: collar ties across rafter pairs. Default 2x4.
        keep collar_tie: Nominal,
        /// Species of the rafters, for the span tables. Default southern
        /// pine.
        fill species: FramingSpecies,
        /// Ground snow load where it's built, pounds per square foot. Ask
        /// the building department. Default 30.
        fill ground_snow_psf: u32,
        /// Weight of the roof itself, pounds per square foot: 10 for
        /// asphalt shingles or metal, 20 for heavier roofing. Default 10.
        fill dead_psf: u32,
        /// Whether a ceiling is fastened to the underside of the rafters.
        /// Default no.
        fill ceiling: bool,
    }
}

/// The side of a building a porch can go on. Only the front for now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PorchSide {
    Front,
}

/// Which structure this one is built against, and on which side.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Against {
    /// The id of a building in this design.
    pub structure: String,
    pub side: PorchSide,
}

spec! {
    /// A covered porch across a building's front.
    pub struct PorchSpec => Porch {
        extra {
            /// A short name for it, like "porch".
            id: String,
            /// The building it's built against.
            against: Against,
            /// A deck under the porch roof, a step below the building's floor.
            /// Leave it out for open ground.
            #[serde(default, skip_serializing_if = "Option::is_none")]
            deck: Option<PorchDeckSpec>,
        }
        /// From the front wall's outside face to the outside of the posts.
        /// Default 6'.
        fill depth: Length,
        /// Post size. Default 4x4.
        fill post: Nominal,
        /// Most room between posts, center to center. Default 8'.
        fill post_spacing: Length,
        /// Beam size. Default 2x8.
        fill beam: Nominal,
        /// Boards laminated into the beam. Default 2.
        fill beam_plies: u8,
        /// Rafter size. Defaults to the building's rafters.
        keep rafter: Nominal,
        /// Defaults to the building's roof pitch.
        keep pitch: Pitch,
        /// Level distance the rafters run past the beam. Default 12".
        fill overhang: Length,
        /// Ledger size. Defaults to two sizes up from the rafter.
        keep ledger: Nominal,
        /// Top of the ledger above the subfloor. Defaults to just under the
        /// front wall's top plates.
        keep ledger_height: Length,
    }
}

spec! {
    /// A deck under a porch: joists hung from the building's floor rim out
    /// to a beam on piers, with decking across them.
    pub struct PorchDeckSpec => PorchDeck {
        extra {}
        /// Joist, rim and ledger size. Defaults to the floor's joists.
        keep joist: Nominal,
        /// Beam size, under the outer rim. Default 4x6.
        fill beam: Nominal,
        /// Decking boards, laid along the front. Default 5/4x6.
        fill decking: Nominal,
        /// Space between decking boards. Default 1/8".
        fill gap: Length,
        /// How far the top of the decking sits below the subfloor.
        /// Default 1".
        fill step_down: Length,
    }
}

spec! {
    /// A deck on its own footings: joists across beams, the beams on posts,
    /// piers or deck blocks, and decking across the joists. Its sizes are
    /// checked against the code's prescriptive deck tables. In plan the
    /// beams run along `width` and the joists along `depth`.
    pub struct DeckSpec => Deck {
        extra {
            /// A short name for it, like "deck".
            id: String,
            /// Along the beams, outside to outside.
            width: Length,
            /// Along the joists, outside to outside.
            depth: Length,
            /// Ground to the top of the decking. Over 30" needs guards, which
            /// Sticks doesn't draw yet.
            height: Length,
        }
        /// Joist and rim size. Default 2x8.
        fill joist: Nominal,
        /// Joist spacing, on center. Defaults to the framing spacing.
        keep on_center: Length,
        /// Beam board size. Default 2x8.
        fill beam: Nominal,
        /// Boards laminated into each beam. Default 2.
        fill beam_plies: u8,
        /// Beam lines across the joists, the outer two near the ends.
        /// Default 2.
        fill beams: u8,
        /// Whether the joists sit on the beams (under) or hang between them
        /// level with them (flush, for a low deck). Default under.
        fill beam_style: BeamStyle,
        /// With beams under the joists: how far the joists run past the
        /// outer beams. Default 0".
        fill overhang: Length,
        /// What the beams sit on. Default piers.
        fill support: DeckSupport,
        /// Post size, when the beams sit on posts. Default 4x4.
        fill post: Nominal,
        /// Most room between supports along a beam. Defaults to the longest
        /// span the code allows.
        keep post_spacing: Length,
        /// From each end of a beam to the center of its end support.
        /// Default 12".
        fill post_inset: Length,
        /// Decking boards, laid across the joists. Default 5/4x6.
        fill decking: Nominal,
        /// Wood or composite decking. Default wood.
        fill decking_material: DeckingMaterial,
        /// Space between decking boards. Default 1/8".
        fill gap: Length,
        /// Which deck rules to check against. Default the 2021 IRC.
        fill code: DeckCode,
        /// Wood species group for the span tables. Default southern pine.
        fill species: Species,
        /// Soil bearing pressure for footing sizes, pounds per square foot.
        /// Default 1500.
        fill soil_psf: u32,
        /// Live load, pounds per square foot. Default 40.
        fill live_psf: u32,
        /// Local frost depth the footings go below. Ask the building
        /// department.
        keep frost_depth: Length,
        /// Pier tops above the ground. Default 2".
        fill pier_above_grade: Length,
    }
}

/// A straight stud wall standing on its own, seen from one face (called
/// outside).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WallSpec {
    /// A short name for it, like "partition". Not front, back, left or right,
    /// which name a building's walls.
    pub id: String,
    pub length: Length,
    /// Overall height, bottom of the bottom plate to top of the top plates.
    pub height: Length,
    /// What happens at the left end. Default plain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left_end: Option<WallEnd>,
    /// What happens at the right end. Default plain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right_end: Option<WallEnd>,
    /// Centers of partitions that tee into this wall, from its left end.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tees: Vec<Length>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub openings: Vec<OpeningSpec>,
}
