//! `sticks schema`, `sticks design` and `sticks validate`: the design schema
//! from the command line.

use std::io::Read;
use std::path::Path;

use sticks_core::Project;
use sticks_schema::{blank_project, claude_schema, from_project, schema as design_schema};

pub fn schema(claude: bool) -> Result<(), String> {
    let s = if claude {
        claude_schema()
    } else {
        design_schema()
    };
    println!("{}", pretty(&s)?);
    Ok(())
}

pub fn export(project: &Project) -> Result<(), String> {
    let d = from_project(project)?;
    println!("{}", pretty(&d)?);
    Ok(())
}

pub fn validate(path: &Path, base: Option<Project>, json: bool, toml: bool) -> Result<(), String> {
    let text = if path == Path::new("-") {
        let mut s = String::new();
        std::io::stdin()
            .read_to_string(&mut s)
            .map_err(|e| format!("can't read standard input: {e}"))?;
        s
    } else {
        std::fs::read_to_string(path).map_err(|e| format!("can't read {}: {e}", path.display()))?
    };
    let base = base.unwrap_or_else(|| blank_project(""));
    let report = sticks_schema::validate(&text, &base);
    if json {
        println!("{}", pretty(&report)?);
    } else if toml {
        if let Some(p) = &report.project {
            print!("{}", p.to_toml().map_err(|e| e.to_string())?);
        }
    } else {
        for n in &report.problems {
            println!("problem  {}: {}", at(&n.path), n.message);
        }
        for n in &report.assumptions {
            println!("assumed  {}: {}", at(&n.path), n.message);
        }
        for q in &report.questions {
            println!("question {q}");
        }
        if report.ok {
            println!("ok");
        }
    }
    if report.ok {
        Ok(())
    } else if json {
        // The report already says why; just fail.
        Err("the design has problems".into())
    } else {
        Err(format!("{} problem(s)", report.problems.len()))
    }
}

fn at(path: &str) -> &str {
    if path.is_empty() {
        "design"
    } else {
        path
    }
}

fn pretty<T: serde::Serialize>(v: &T) -> Result<String, String> {
    serde_json::to_string_pretty(v).map_err(|e| e.to_string())
}
