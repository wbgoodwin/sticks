//! The design schema: every structure Sticks can frame, written as JSON that
//! a person, Claude or a classifier can fill in.
//!
//! A [`Design`] is a list of [`Structure`]s, each a typed kind with only the
//! choices Sticks can frame and check. Most fields are optional: left out,
//! Sticks picks its documented default and lists it as an assumption, so a
//! design written from a short request says only what the request said.
//!
//! The Rust types are the source of truth. [`schema`] is their JSON Schema,
//! [`claude_schema`] the same trimmed to what Claude's structured outputs
//! accept, and [`validate`] reads a design back: shape, values and fit, then
//! whether it frames.

mod convert;
mod spec;
mod types;
mod validate;

pub use convert::{blank_project, from_project, to_project, Converted};
pub use types::*;
pub use validate::{claude_schema, schema, validate, Note, Report};

/// The version every design written against this schema names.
pub const VERSION: &str = "sticks/1";
