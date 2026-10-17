//! Design changes in plain English, turned into typed edits.
//!
//! Rust reads the notation (clauses, lengths, pitches, lumber sizes) and asks
//! a classifier such as Jev a fixed batch of multiple-choice questions about
//! the meaning ([`questions`]). Confident answers become typed edits; the
//! rest come back as a [`Question`] for the user. Answers go back in by
//! question id and the request is read again, so nothing is kept between
//! calls. Rust then places, checks and frames the result.

mod edit;
mod interpret;
mod words;

pub use edit::{Dimension, Edit, EditError, Framing, Side};
pub use interpret::{
    interpret, questions, Answer, Answers, Ask, Choice, Interpretation, Question, Step, CONFIDENT,
};

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
