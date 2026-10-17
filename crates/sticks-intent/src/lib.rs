//! Design changes in plain English, turned into typed edits.
//!
//! Rust settles everything it can: numbers, units, which part a sentence is
//! about, and whether the result still frames. What words alone can't settle
//! comes back as a [`Question`] with fixed options, for a classifier (Jev, or
//! a local model) or the user to answer. Answers go back in by question id and
//! the request is read again, so nothing is kept between calls.

mod edit;
mod interpret;
mod words;

pub use edit::{Dimension, Edit, EditError, Framing, Side};
pub use interpret::{interpret, Answers, Choice, Interpretation, Question, Step};

use sticks_core::Project;
use sticks_rules::frame_project;

/// The project with `edits` applied, in order.
pub fn apply_all(project: &Project, edits: &[Edit]) -> Result<Project, EditError> {
    let mut p = project.clone();
    for e in edits {
        e.apply(&mut p)?;
    }
    Ok(p)
}

/// Why a project won't frame, if it won't.
pub fn framing_problems(project: &Project) -> Vec<String> {
    match frame_project(project) {
        Ok(f) => f.errors().iter().map(|e| e.to_string()).collect(),
        Err(e) => vec![e.to_string()],
    }
}
