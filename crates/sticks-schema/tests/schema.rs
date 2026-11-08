use std::path::PathBuf;

use serde_json::Value;
use sticks_core::{Length, OpeningKind, Project, RoofKind};
use sticks_schema::{
    blank_project, claude_schema, from_project, schema, to_project, validate, Design,
};

fn example(name: &str) -> Project {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(name);
    Project::from_toml(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Project -> design -> JSON -> validate -> the same project.
fn round_trips(name: &str) {
    let p = example(name);
    let d = from_project(&p).unwrap();
    let json = serde_json::to_string_pretty(&d).unwrap();
    let r = validate(&json, &p);
    assert!(r.ok, "{name}: {:?}", r.problems);
    assert!(
        r.assumptions.is_empty(),
        "{name}: a spelled-out design assumes nothing: {:?}",
        r.assumptions
    );
    assert_eq!(r.project.unwrap(), p, "{name}");
    assert_eq!(r.design.unwrap(), d);
}

#[test]
fn the_shed_round_trips() {
    round_trips("shed.toml");
}

#[test]
fn the_deck_round_trips() {
    round_trips("deck.toml");
}

#[test]
fn a_freestanding_wall_round_trips() {
    let p = Project::from_toml(
        r#"
name = "Garden wall"
[[wall]]
id = "W1"
length = "18'"
height = "10'"
left_end = "corner"
tees = ["11'"]
[[wall.opening]]
kind = "door"
offset = "6' 3\""
width = "38-1/2\""
height = "82-1/2\""
header = "2x8"
"#,
    )
    .unwrap();
    let d = from_project(&p).unwrap();
    let back = to_project(&d, &p).unwrap();
    assert_eq!(back.project, p);
}

const SMALL: &str = r#"{
  "schema": "sticks/1",
  "name": "10x12 shed",
  "structures": [{
    "kind": "building", "id": "shed",
    "width": "10'", "depth": "12'", "height": "8'",
    "floor": { "support": "skids" },
    "roof": { "kind": "gable" },
    "walls": [
      { "side": "front", "openings": [{ "kind": "door", "place": { "from": "center" } }] },
      { "side": "left", "openings": [{ "kind": "window", "place": { "from": "right", "distance": "2'" } }] }
    ]
  }],
  "questions": ["What pitch should the roof be?"]
}"#;

#[test]
fn a_short_design_fills_in_and_lists_its_defaults() {
    let r = validate(SMALL, &blank_project(""));
    assert!(r.ok, "{:?}", r.problems);
    let p = r.project.unwrap();
    let roof = p.roof.unwrap();
    assert_eq!(roof.kind, RoofKind::Gable);
    assert_eq!(roof.pitch.to_string(), "4:12");
    // A 38-1/2" door centered on the 10' front wall.
    let door = &p.walls[0].openings[0];
    assert_eq!(door.kind, OpeningKind::Door);
    assert_eq!(door.offset, Length::from_sixteenths((120 * 16 - 616) / 2));
    // A 36" window 2' from the left wall's right end.
    let window = &p.walls[1].openings[0];
    assert_eq!(
        window.offset,
        Length::feet(12) - Length::feet(2) - Length::inches(36)
    );
    let assumed: Vec<&str> = r.assumptions.iter().map(|n| n.path.as_str()).collect();
    assert!(assumed.contains(&"structures[0].roof.pitch"), "{assumed:?}");
    assert!(assumed.contains(&"framing.on_center"), "{assumed:?}");
    assert!(!assumed.contains(&"structures[0].floor.support"));
    assert_eq!(
        r.questions,
        vec!["What pitch should the roof be?".to_string()]
    );
}

#[test]
fn settings_outside_the_design_come_from_the_base() {
    let base = example("shed.toml");
    let r = validate(SMALL, &base);
    let p = r.project.unwrap();
    assert_eq!(p.stock, base.stock);
    assert_eq!(p.rules.kerf, base.rules.kerf);
    assert!(
        p.porch.is_none(),
        "the design replaces the base's structures"
    );
}

fn problems(json: &str) -> Vec<(String, String)> {
    let r = validate(json, &blank_project(""));
    assert!(!r.ok);
    r.problems
        .into_iter()
        .map(|n| (n.path, n.message))
        .collect()
}

fn with(structures: &str) -> String {
    format!(r#"{{"schema": "sticks/1", "name": "x", "structures": [{structures}]}}"#)
}

#[test]
fn a_bad_length_names_its_field() {
    let p = problems(&with(
        r#"{"kind": "building", "id": "s", "width": "3 ft", "depth": "12'", "height": "8'"}"#,
    ));
    assert_eq!(p[0].0, "structures[0].width");
    assert!(p[0].1.contains("3 ft"), "{}", p[0].1);
}

#[test]
fn an_unknown_field_names_where_it_is() {
    let p = problems(&with(
        r#"{"kind": "deck", "id": "d", "width": "12'", "depth": "10'", "height": "2'", "colour": "red"}"#,
    ));
    assert_eq!(p[0].0, "structures[0].colour");
}

#[test]
fn an_unknown_kind_is_refused() {
    let p = problems(&with(r#"{"kind": "pergola", "id": "p"}"#));
    assert_eq!(p[0].0, "structures[0].kind");
    assert!(p[0].1.contains("pergola"), "{}", p[0].1);
}

#[test]
fn openings_have_to_fit_their_wall() {
    let p = problems(&with(
        r#"{"kind": "wall", "id": "w", "length": "10'", "height": "8'", "openings": [
            {"kind": "door", "place": {"from": "left", "distance": "9'"}},
            {"kind": "window", "place": {"from": "left", "distance": "8' 6\""}, "height": "7'"}
        ]}"#,
    ));
    let paths: Vec<&str> = p.iter().map(|(path, _)| path.as_str()).collect();
    assert!(paths.contains(&"structures[0].openings[0].place"), "{p:?}");
    assert!(paths.contains(&"structures[0].openings[1].height"), "{p:?}");
}

#[test]
fn a_porch_needs_its_building() {
    let p = problems(&with(
        r#"{"kind": "porch", "id": "p", "against": {"structure": "shed", "side": "front"}}"#,
    ));
    assert_eq!(p[0].0, "structures[0].against.structure");
}

#[test]
fn one_building_per_design_for_now() {
    let b = r#"{"kind": "building", "id": "a", "width": "8'", "depth": "8'", "height": "8'"}"#;
    let b2 = b.replace("\"a\"", "\"b\"");
    let p = problems(&with(&format!("{b}, {b2}")));
    assert_eq!(p[0].0, "structures[1]");
}

#[test]
fn ids_are_unique() {
    let p = problems(&with(
        r#"{"kind": "wall", "id": "w", "length": "8'", "height": "8'"},
           {"kind": "wall", "id": "w", "length": "8'", "height": "8'"}"#,
    ));
    assert_eq!(p[0].0, "structures[1].id");
}

#[test]
fn designs_that_wont_frame_say_so() {
    // A gable roof too steep for Sticks.
    let p = problems(&with(
        r#"{"kind": "building", "id": "s", "width": "8'", "depth": "8'", "height": "8'", "roof": {"pitch": "30:12"}}"#,
    ));
    assert!(p[0].1.contains("30:12"), "{p:?}");
}

#[test]
fn the_schema_reads_back_what_it_describes() {
    // The four kinds Sticks frames today, in order.
    let s = schema();
    assert_eq!(s["title"], "Sticks design");
    let kinds: Vec<String> = s["$defs"]["Structure"]["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| {
            k["properties"]["kind"]["const"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();
    assert_eq!(kinds, ["building", "porch", "deck", "wall"]);
}

#[test]
fn the_claude_schema_uses_only_what_structured_outputs_accept() {
    fn walk(v: &Value, path: &str) {
        match v {
            Value::Object(m) => {
                for bad in [
                    "oneOf", "minimum", "maximum", "format", "default", "examples",
                ] {
                    assert!(!m.contains_key(bad), "{bad} at {path}");
                }
                if m.get("type").is_some_and(|t| t == "object") {
                    assert_eq!(
                        m.get("additionalProperties"),
                        Some(&Value::Bool(false)),
                        "{path}"
                    );
                }
                for (k, c) in m {
                    walk(c, &format!("{path}/{k}"));
                }
            }
            Value::Array(a) => a
                .iter()
                .enumerate()
                .for_each(|(i, c)| walk(c, &format!("{path}/{i}"))),
            _ => {}
        }
    }
    walk(&claude_schema(), "");
}

/// The schema is checked in at `schema/`, so a change to it shows in review.
/// Run with `UPDATE_SCHEMA=1` to write it again.
#[test]
fn the_checked_in_schema_is_current() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schema");
    for (file, value) in [
        ("design.schema.json", schema()),
        ("design.claude.json", claude_schema()),
    ] {
        let text = serde_json::to_string_pretty(&value).unwrap() + "\n";
        let path = dir.join(file);
        if std::env::var_os("UPDATE_SCHEMA").is_some() {
            std::fs::write(&path, &text).unwrap();
        }
        let on_disk = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            on_disk == text,
            "{file} is out of date; run UPDATE_SCHEMA=1 cargo test -p sticks-schema"
        );
    }
}

#[test]
fn a_design_written_by_hand_parses() {
    let d: Design = serde_json::from_str(SMALL).unwrap();
    assert_eq!(d.structures.len(), 1);
}
