//! Core types for Sticks: fixed-point lengths, the structure model,
//! and project-file loading. No I/O beyond parsing strings.

pub mod lumber;
pub mod project;
pub mod units;

pub use lumber::{
    longest_board, tenths_of_degree, Cut, Member, MemberKind, Nominal, SheetPiece,
    DEFAULT_STOCK_FEET,
};
pub use project::{
    Building, CornerStyle, Floor, FloorSupport, LayoutStart, Opening, OpeningKind, Pitch, Project,
    ProjectError, Roof, RoofKind, Rules, SheetStock, Stock, Wall, WallEnd,
};
pub use units::{Length, ParseLengthError};
