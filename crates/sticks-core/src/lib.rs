//! Core types for Sticks: fixed-point lengths, the structure model,
//! and project-file loading. No I/O beyond parsing strings.

pub mod lumber;
pub mod project;
pub mod units;

pub use lumber::{Cut, Member, MemberKind, Nominal};
pub use project::{
    CornerStyle, LayoutStart, Opening, OpeningKind, Project, ProjectError, Rules, Stock, Wall,
    WallEnd,
};
pub use units::{Length, ParseLengthError};
