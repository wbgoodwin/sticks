//! The bridge between the Rust core and the TypeScript app.
//!
//! Everything crosses as JSON strings. Lengths go out as whole sixteenths
//! (`*_16`) with a display string beside them, so the app draws from exact
//! numbers and never formats fractions itself.

use serde::Serialize;
use sticks_core::{Length, Member, MemberKind, Project};
use sticks_cutlist::{cut_list_with_sheets, CutList};
use sticks_rules::{frame_project, FramedWall, SIDES};
use wasm_bindgen::prelude::*;

/// Frames every wall in a project given as JSON (the same shape as the TOML
/// file; lengths may be builder strings like `"92-5/8\""`). A project with a
/// building lists its front, back, left and right walls first.
#[wasm_bindgen]
pub fn frame(project_json: &str) -> Result<String, JsError> {
    frame_json(project_json).map_err(|e| JsError::new(&e))
}

/// Plans the stock boards for every wall that frames, and how to cut them.
#[wasm_bindgen]
pub fn cut_list(project_json: &str) -> Result<String, JsError> {
    cut_list_json(project_json).map_err(|e| JsError::new(&e))
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
    left_height_16: i64,
    right_height_16: i64,
    members: Vec<MemberView>,
    marks: Vec<MarkView>,
    splices: Vec<SpliceView>,
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
    /// Corners as `[x, y]` for members that aren't rectangles; else empty.
    outline_16: Vec<[i64; 2]>,
}

#[derive(Serialize)]
struct SpliceView {
    kind: MemberKind,
    at_16: i64,
    at: String,
}

#[derive(Serialize)]
struct MarkView {
    label: String,
    kind: MemberKind,
    at_16: i64,
    at: String,
}

#[derive(Serialize)]
struct CutListView {
    boards: Vec<BoardView>,
    bom: Vec<BomView>,
    too_long: Vec<PieceView>,
    summary: SummaryView,
    /// Parts left out because they don't frame, such as `wall front`.
    skipped: Vec<String>,
}

#[derive(Serialize)]
struct BoardView {
    id: String,
    size: String,
    length_16: i64,
    length: String,
    price_cents: Option<u32>,
    pieces: Vec<PieceView>,
    offcut_16: i64,
    offcut: String,
}

#[derive(Serialize)]
struct PieceView {
    label: String,
    kind: MemberKind,
    size: String,
    length_16: i64,
    length: String,
}

#[derive(Serialize)]
struct BomView {
    /// A lumber size like `2x4`, or a sheet like `3/4" sheet`.
    size: String,
    length_16: i64,
    length: String,
    count: usize,
    unit_price_cents: Option<u32>,
    total_cents: Option<u64>,
}

#[derive(Serialize)]
struct SummaryView {
    boards: usize,
    sheets: usize,
    pieces: usize,
    total_cents: Option<u64>,
    waste_percent: f64,
    kerf: String,
}

pub fn cut_list_json(project_json: &str) -> Result<String, String> {
    let project: Project = serde_json::from_str(project_json).map_err(|e| e.to_string())?;
    let framed = frame_project(&project).map_err(|e| e.to_string())?;
    let skipped = framed.errors().iter().map(|e| e.part.clone()).collect();
    let cl = cut_list_with_sheets(
        &framed.members(),
        &framed.sheet_pieces(),
        &project.stock,
        &project.sheets,
        &project.rules,
    )
    .map_err(|e| e.to_string())?;
    serde_json::to_string(&cut_list_view(cl, skipped)).map_err(|e| e.to_string())
}

fn cut_list_view(cl: CutList, skipped: Vec<String>) -> CutListView {
    let s = cl.summary();
    let mut bom: Vec<BomView> = cl
        .bom()
        .into_iter()
        .map(|l| BomView {
            size: l.size.to_string(),
            length_16: l.length.sixteenths(),
            length: l.length.to_string(),
            count: l.count,
            unit_price_cents: l.unit_price_cents,
            total_cents: l.total_cents,
        })
        .collect();
    bom.extend(cl.sheet_bom().into_iter().map(|l| BomView {
        size: format!("{:#} sheet", l.thickness),
        length_16: l.length.sixteenths(),
        length: format!("{} x {}", l.width, l.length),
        count: l.count,
        unit_price_cents: l.unit_price_cents,
        total_cents: l.total_cents,
    }));
    CutListView {
        summary: SummaryView {
            boards: s.boards,
            sheets: s.sheets,
            pieces: s.pieces,
            total_cents: s.total_cents,
            waste_percent: s.waste_percent,
            kerf: format!("{:#}", cl.kerf),
        },
        bom,
        boards: cl
            .boards
            .into_iter()
            .map(|b| BoardView {
                id: b.id,
                size: b.size.to_string(),
                length_16: b.length.sixteenths(),
                length: b.length.to_string(),
                price_cents: b.price_cents,
                pieces: b.pieces.into_iter().map(piece_view).collect(),
                offcut_16: b.offcut.sixteenths(),
                offcut: format!("{:#}", b.offcut),
            })
            .collect(),
        too_long: cl.too_long.into_iter().map(piece_view).collect(),
        skipped,
    }
}

fn piece_view(m: Member) -> PieceView {
    PieceView {
        length: m.length.to_tape_string(),
        label: m.label,
        kind: m.kind,
        size: m.size.to_string(),
        length_16: m.length.sixteenths(),
    }
}

pub fn frame_json(project_json: &str) -> Result<String, String> {
    let project: Project = serde_json::from_str(project_json).map_err(|e| e.to_string())?;
    let framed = frame_project(&project).map_err(|e| e.to_string())?;
    // Walls come back in the order frame_project lists them.
    let mut ids: Vec<String> = Vec::new();
    if project.building.is_some() {
        ids.extend(SIDES.iter().map(|s| s.to_string()));
    }
    ids.extend(
        project
            .walls
            .iter()
            .filter(|w| project.building.is_none() || !SIDES.contains(&w.id.as_str()))
            .map(|w| w.id.clone()),
    );
    let walls = framed
        .walls
        .into_iter()
        .zip(ids)
        .map(|(result, id)| match result {
            Ok(framed) => wall_view(framed),
            Err(e) => {
                let wall = project.walls.iter().find(|w| w.id == id);
                let length = wall.and_then(|w| w.length).unwrap_or_default();
                let height = wall.and_then(|w| w.height).unwrap_or_default();
                WallView {
                    id,
                    length_16: length.sixteenths(),
                    height_16: height.sixteenths(),
                    left_height_16: height.sixteenths(),
                    right_height_16: height.sixteenths(),
                    members: Vec::new(),
                    marks: Vec::new(),
                    splices: Vec::new(),
                    warnings: Vec::new(),
                    error: Some(e.reason),
                }
            }
        })
        .collect();
    serde_json::to_string(&FrameView { walls }).map_err(|e| e.to_string())
}

fn wall_view(w: FramedWall) -> WallView {
    WallView {
        id: w.id,
        length_16: w.length.sixteenths(),
        height_16: w.height.sixteenths(),
        left_height_16: w.left_height.sixteenths(),
        right_height_16: w.right_height.sixteenths(),
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
                outline_16: p
                    .outline
                    .iter()
                    .map(|(x, y)| [x.sixteenths(), y.sixteenths()])
                    .collect(),
            })
            .collect(),
        splices: w
            .splices
            .into_iter()
            .map(|s| SpliceView {
                kind: s.kind,
                at_16: s.at.sixteenths(),
                at: format!("{:#}", s.at),
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
    fn plans_cuts_from_json() {
        let json = r#"{
            "name": "x",
            "rules": {"kerf": "1/8\""},
            "stock": [{"size": "2x4", "lengths": ["8'", "16'"], "prices_cents": [498, 1149]}],
            "wall": [
                {"id": "A", "length": "8'", "height": "97-1/8\""},
                {"id": "B", "length": "8'", "height": "2\""}
            ]
        }"#;
        let out: serde_json::Value = serde_json::from_str(&cut_list_json(json).unwrap()).unwrap();
        assert_eq!(out["skipped"][0], "wall B");
        assert!(out["too_long"].as_array().unwrap().is_empty());
        let pieces: usize = out["boards"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| b["pieces"].as_array().unwrap().len())
            .sum();
        assert_eq!(pieces, out["summary"]["pieces"]);
        assert!(out["summary"]["total_cents"].is_u64());
        assert_eq!(out["bom"][0]["size"], "2x4");
    }

    #[test]
    fn frames_a_whole_building() {
        let json = project_from_toml(include_str!("../../../examples/shed.toml")).unwrap();
        let out: serde_json::Value = serde_json::from_str(&frame_json(&json).unwrap()).unwrap();
        let ids: Vec<_> = out["walls"]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| w["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, SIDES);
        let left = &out["walls"][2];
        assert!(left["error"].is_null());
        assert_ne!(left["left_height_16"], left["right_height_16"]);
        assert_eq!(
            left["members"][1]["outline_16"].as_array().unwrap().len(),
            4
        );
        assert!(!out["walls"][0]["splices"].as_array().unwrap().is_empty());

        let plan: serde_json::Value = serde_json::from_str(&cut_list_json(&json).unwrap()).unwrap();
        assert!(plan["too_long"].as_array().unwrap().is_empty());
        assert!(plan["summary"]["sheets"].as_u64().unwrap() >= 8);
        assert!(plan["bom"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["size"] == "3/4\" sheet"));
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
