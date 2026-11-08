//! Reading a design back, and the JSON Schema it's written against.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sticks_core::Project;
use sticks_rules::frame_project;

use crate::convert::to_project;
use crate::types::Design;

/// Something about one place in a design: a problem, or a default it took.
/// `path` is like `structures[0].walls[1].openings[0].width`; empty for the
/// design as a whole.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub path: String,
    pub message: String,
}

/// What [`validate`] found.
#[derive(Clone, Debug, Serialize)]
pub struct Report {
    /// True when the design reads, fits and frames.
    pub ok: bool,
    /// Why it can't be used, each with a plain fix. Sent back to a model as
    /// is, and shown to the person if a retry still fails.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub problems: Vec<Note>,
    /// Defaults Sticks picked for what the design left out.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub assumptions: Vec<Note>,
    /// Questions the design asks the person.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub questions: Vec<String>,
    /// The design, when it read.
    #[serde(skip)]
    pub design: Option<Design>,
    /// The project it becomes, when it's ok.
    #[serde(skip)]
    pub project: Option<Project>,
}

impl Report {
    fn failed(problems: Vec<Note>) -> Report {
        Report {
            ok: false,
            problems,
            assumptions: Vec::new(),
            questions: Vec::new(),
            design: None,
            project: None,
        }
    }
}

/// Reads `json` as a design and checks it in four steps: shape (it parses,
/// with no unknown fields), values (lengths, sizes and pitches read), fit
/// (openings inside their walls, ids unique, a porch against a real
/// building), then whether it frames. Stock, prices and other settings a
/// design doesn't hold come from `base`.
pub fn validate(json: &str, base: &Project) -> Report {
    let de = &mut serde_json::Deserializer::from_str(json);
    let design: Design = match serde_path_to_error::deserialize(de) {
        Ok(d) => d,
        Err(e) => {
            let path = e.path().to_string();
            let path = if path == "." { String::new() } else { path };
            let note = Note {
                path,
                message: plain(&e.into_inner()),
            };
            return Report::failed(vec![inside_structure(json, note)]);
        }
    };
    let converted = match to_project(&design, base) {
        Ok(c) => c,
        Err(problems) => {
            let mut r = Report::failed(problems);
            r.questions = design.questions.clone();
            r.design = Some(design);
            return r;
        }
    };
    let problems: Vec<Note> = match frame_project(&converted.project) {
        Ok(f) => f
            .errors()
            .iter()
            .map(|e| Note {
                path: String::new(),
                message: e.to_string(),
            })
            .collect(),
        Err(e) => vec![Note {
            path: String::new(),
            message: e.to_string(),
        }],
    };
    let ok = problems.is_empty();
    Report {
        ok,
        problems,
        assumptions: converted.assumptions,
        questions: design.questions.clone(),
        design: Some(design),
        project: ok.then_some(converted.project),
    }
}

/// A serde error without the line and column, which mean nothing to a
/// model that wrote one long line.
fn plain(e: &serde_json::Error) -> String {
    let s = e.to_string();
    match s.rfind(" at line ") {
        Some(i) => s[..i].to_string(),
        None => s,
    }
}

/// A structure's kind picks its fields, so serde reads it whole and an
/// error inside one stops at `structures[i]`. Reading that one again as its
/// own kind finds the field.
fn inside_structure(json: &str, note: Note) -> Note {
    let Some(i) = note
        .path
        .strip_prefix("structures[")
        .and_then(|rest| rest.strip_suffix(']'))
        .and_then(|n| n.parse::<usize>().ok())
    else {
        return note;
    };
    let Ok(Value::Object(mut d)) = serde_json::from_str::<Value>(json) else {
        return note;
    };
    let Some(Value::Object(mut s)) = d
        .remove("structures")
        .and_then(|v| v.as_array().and_then(|a| a.get(i).cloned()))
    else {
        return note;
    };
    let kind = s.remove("kind");
    let text = Value::Object(s).to_string();
    let found = match kind.as_ref().and_then(Value::as_str) {
        Some("building") => inner::<crate::BuildingSpec>(&text),
        Some("porch") => inner::<crate::PorchSpec>(&text),
        Some("deck") => inner::<crate::DeckSpec>(&text),
        Some("wall") => inner::<crate::WallSpec>(&text),
        _ => None,
    };
    match found {
        Some((path, message)) => Note {
            path: if path.is_empty() {
                note.path
            } else {
                format!("{}.{path}", note.path)
            },
            message,
        },
        None => note,
    }
}

fn inner<T: serde::de::DeserializeOwned>(text: &str) -> Option<(String, String)> {
    let de = &mut serde_json::Deserializer::from_str(text);
    match serde_path_to_error::deserialize::<_, T>(de) {
        Ok(_) => None,
        Err(e) => {
            let path = e.path().to_string();
            let path = if path == "." { String::new() } else { path };
            Some((path, plain(&e.into_inner())))
        }
    }
}

/// The design's JSON Schema, as generated from the Rust types.
pub fn schema() -> Value {
    serde_json::to_value(schemars::schema_for!(Design)).expect("a schema is JSON")
}

/// String formats Claude's structured outputs accept.
const CLAUDE_FORMATS: [&str; 10] = [
    "date-time",
    "time",
    "date",
    "duration",
    "email",
    "hostname",
    "uri",
    "ipv4",
    "ipv6",
    "uuid",
];

/// Keywords Claude's structured outputs don't enforce. Rust checks ranges
/// itself, so dropping them loses nothing.
const CLAUDE_DROPPED: [&str; 12] = [
    "minimum",
    "maximum",
    "exclusiveMinimum",
    "exclusiveMaximum",
    "multipleOf",
    "minLength",
    "maxLength",
    "pattern",
    "minItems",
    "maxItems",
    "default",
    "examples",
];

/// [`schema`] trimmed for Claude's structured outputs (`output_config.format`):
/// `oneOf` becomes `anyOf`, numeric and length limits and unsupported
/// formats go, and every object closes with `additionalProperties: false`.
pub fn claude_schema() -> Value {
    let mut s = schema();
    trim_for_claude(&mut s);
    s
}

fn trim_for_claude(v: &mut Value) {
    match v {
        Value::Object(map) => {
            if let Some(one_of) = map.remove("oneOf") {
                map.insert("anyOf".into(), one_of);
            }
            for k in CLAUDE_DROPPED {
                map.remove(k);
            }
            if let Some(Value::String(f)) = map.get("format") {
                if !CLAUDE_FORMATS.contains(&f.as_str()) {
                    map.remove("format");
                }
            }
            close_object(map);
            for (_, child) in map.iter_mut() {
                trim_for_claude(child);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(trim_for_claude),
        _ => {}
    }
}

fn close_object(map: &mut Map<String, Value>) {
    let is_object =
        map.get("type").is_some_and(|t| t == "object") || map.contains_key("properties");
    if is_object && !map.contains_key("additionalProperties") {
        map.insert("additionalProperties".into(), Value::Bool(false));
    }
}
