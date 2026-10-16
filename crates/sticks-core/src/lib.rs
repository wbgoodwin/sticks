//! Core types for Sticks: fixed-point lengths, the structure model,
//! and project-file loading. No I/O beyond parsing strings.

pub mod lumber;
pub mod project;
pub mod units;

pub use lumber::{Cut, Member, MemberKind, Nominal};
pub use project::{Project, ProjectError};
pub use units::{Length, ParseLengthError};
