//! The bridge between the Rust core and the TypeScript app.
//!
//! Everything crosses as JSON strings. Lengths go out as whole sixteenths
//! (`*_16`) with a display string beside them, so the app draws from exact
//! numbers and never formats fractions itself.

use serde::Serialize;
use sticks_core::{Length, MemberKind, Project};
use sticks_rules::{frame_wall, FramedWall};
use wasm_bindgen::prelude::*;

/// Frames every wall in a project given as JSON (the same shape as the TOML
/// file; lengths may be builder strings like `"92-5/8\""`).
#[wasm_bindgen]
pub fn frame(project_json: &str) -> Result<String, JsError> {
    frame_json(project_json).map_err(|e| JsError::new(&e))
}

/// Reads a TOML project file into the JSON the app edits.
#[wasm_bindgen]
pub fn project_from_toml(text: &str) -> Result<String, JsError> {
    let project = Project::from_toml(text).map_err(|e| JsError::new(&e.to_string()))?;
    serde_json::to_string(&project).map_err(|e| JsError::new(&e.to_string()))
}

/// Writes the app's JSON project back out as a TOML project file.
#[wasm_bindgen]
pub fn project_to_toml(project_json: &str) -> Result<String, JsError> {
    let project: Project =
        serde_json::from_str(project_json).map_err(|e| JsError::new(&e.to_string()))?;
    project.to_toml().map_err(|e| JsError::new(&e.to_string()))
}

/// Parses builder notation into sixteenths of an inch.
#[wasm_bindgen]
pub fn parse_length(text: &str) -> Result<i32, JsError> {
    let l: Length = text
        .parse()
        .map_err(|e: sticks_core::ParseLengthError| JsError::new(&e.to_string()))?;
    i32::try_from(l.sixteenths()).map_err(|_| JsError::new("that length is too large"))
}

/// Formats sixteenths as feet and inches (`10' 3-1/2"`), or inches only.
#[wasm_bindgen]
pub fn format_length(sixteenths: i32, inches_only: bool) -> String {
    let l = Length::from_sixteenths(sixteenths.into());
    if inches_only {
        format!("{l:#}")
    } else {
        l.to_string()
    }
}

#[derive(Serialize)]
struct FrameView {
    walls: Vec<WallView>,
}

#[derive(Serialize)]
struct WallView {
    id: String,
    length_16: i64,
    height_16: i64,
    members: Vec<MemberView>,
    marks: Vec<MarkView>,
    warnings: Vec<String>,
    /// Set when the wall can't be framed; the other fields are then empty.
    error: Option<String>,
}

#[derive(Serialize)]
struct MemberView {
    label: String,
    kind: MemberKind,
    size: String,
    length_16: i64,
    length: String,
    x_16: i64,
    y_16: i64,
    w_16: i64,
    h_16: i64,
}

#[derive(Serialize)]
struct MarkView {
    label: String,
    kind: MemberKind,
    at_16: i64,
    at: String,
}

pub fn frame_json(project_json: &str) -> Result<String, String> {
    let project: Project = serde_json::from_str(project_json).map_err(|e| e.to_string())?;
    let walls = project
        .walls
        .iter()
        .map(|wall| match frame_wall(wall, &project.rules) {
            Ok(framed) => wall_view(framed),
            Err(e) => WallView {
                id: wall.id.clone(),
                length_16: wall.length.sixteenths(),
                height_16: wall.height.sixteenths(),
                members: Vec::new(),
                marks: Vec::new(),
                warnings: Vec::new(),
                error: Some(e.reason),
            },
        })
        .collect();
    serde_json::to_string(&FrameView { walls }).map_err(|e| e.to_string())
}

fn wall_view(w: FramedWall) -> WallView {
    WallView {
        id: w.id,
        length_16: w.length.sixteenths(),
        height_16: w.height.sixteenths(),
        members: w
            .members
            .into_iter()
            .map(|p| MemberView {
                length: p.member.length.to_tape_string(),
                label: p.member.label,
                kind: p.member.kind,
                size: p.member.size.to_string(),
                length_16: p.member.length.sixteenths(),
                x_16: p.rect.x.sixteenths(),
                y_16: p.rect.y.sixteenths(),
                w_16: p.rect.w.sixteenths(),
                h_16: p.rect.h.sixteenths(),
            })
            .collect(),
        marks: w
            .marks
            .into_iter()
            .map(|m| MarkView {
                at: format!("{:#}", m.at),
                label: m.label,
                kind: m.kind,
                at_16: m.at.sixteenths(),
            })
            .collect(),
        warnings: w.warnings,
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_json_with_builder_lengths() {
        let json = r#"{
            "name": "x",
            "wall": [
                {"id": "A", "length": "8'", "height": "97-1/8\""},
                {"id": "B", "length": "8'", "height": "2\""}
            ]
        }"#;
        let out: serde_json::Value = serde_json::from_str(&frame_json(json).unwrap()).unwrap();
        let a = &out["walls"][0];
        assert_eq!(a["length_16"], 96 * 16);
        assert_eq!(a["members"][3]["length"], "92-5/8\"");
        assert!(a["error"].is_null());
        assert!(out["walls"][1]["error"]
            .as_str()
            .unwrap()
            .contains("too short"));
    }

    #[test]
    fn toml_round_trips_through_json() {
        let toml = include_str!("../../../examples/shed.toml");
        let json = project_from_toml(toml).unwrap();
        let back = project_to_toml(&json).unwrap();
        assert_eq!(
            Project::from_toml(&back).unwrap(),
            Project::from_toml(toml).unwrap()
        );
    }
}
