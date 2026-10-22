//! Core types for Sticks: fixed-point lengths, the structure model,
//! and project-file loading. No I/O beyond parsing strings.

pub mod joint;
pub mod lumber;
pub mod project;
pub mod units;

pub use joint::{Joint, JointKind};
pub use lumber::{
    longest_board, tenths_of_degree, Cut, Member, MemberKind, Nominal, SheetPiece, SheetUse,
    DEFAULT_STOCK_FEET,
};
pub use project::{
    BeamStyle, Building, CornerStyle, Deck, DeckCode, DeckSupport, DeckingMaterial, Fastening,
    Floor, FloorSupport, Hardware, LayoutStart, Opening, OpeningKind, Pitch, Porch, PorchDeck,
    Project, ProjectError, Roof, RoofKind, Rules, Sheathing, SheetStock, Species, Stock, Wall,
    WallEnd,
};
pub use units::{Length, ParseLengthError};
