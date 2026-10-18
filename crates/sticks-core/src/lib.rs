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
    Building, CornerStyle, Deck, Fastening, Floor, FloorSupport, Hardware, LayoutStart, Opening,
    OpeningKind, Pitch, Porch, Project, ProjectError, Roof, RoofKind, Rules, Sheathing, SheetStock,
    Stock, Wall, WallEnd,
};
pub use units::{Length, ParseLengthError};
