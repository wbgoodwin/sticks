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
    BeamStyle, Building, CornerStyle, Deck, DeckAttach, DeckCode, DeckSide, DeckSite, DeckStairs,
    DeckSupport, DeckingMaterial, Fastening, Floor, FloorSupport, FramingSpecies, GuardRule,
    Hardware, LayoutStart, LedgerFasteners, Opening, OpeningKind, Pitch, Porch, PorchDeck, Project,
    ProjectError, Roof, RoofKind, Rules, Sheathing, SheetStock, SiteHouse, Species, Stock, Wall,
    WallEnd, DEFAULT_GROUND_SNOW_PSF, DEFAULT_ROOF_DEAD_PSF,
};
pub use units::{Length, ParseLengthError};
