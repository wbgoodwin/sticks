//! `sticks edit`: read a plain-English request, show what it would change,
//! and save it once nothing is left to ask.

use std::path::{Path, PathBuf};

use serde_json::json;
use sticks_core::Project;
use sticks_intent::{
    apply_all, framing_problems, interpret, questions, Answer, Answers, Interpretation,
};

pub struct Opts {
    pub questions: bool,
    pub answers_file: Option<PathBuf>,
    pub answers: Vec<String>,
    pub json: bool,
    pub write: bool,
}

pub fn run(project: &Project, path: &Path, request: &str, opts: &Opts) -> Result<(), String> {
    let mut answers = match &opts.answers_file {
        Some(f) => {
            let text = std::fs::read_to_string(f)
                .map_err(|e| format!("can't read {}: {e}", f.display()))?;
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", f.display()))?
        }
        None => Answers::new(),
    };
    answers.extend(parse_answers(&opts.answers)?);
    if opts.questions {
        let asks = serde_json::to_string_pretty(&questions(project, request, &answers))
            .map_err(|e| e.to_string())?;
        println!("{asks}");
        return Ok(());
    }
    let (as_json, write) = (opts.json, opts.write);
    let reading = interpret(project, request, &answers);
    let preview = match reading.edits() {
        Some(edits) => Some(preview(project, &edits)?),
        None => None,
    };

    if as_json {
        let out = json!({
            "steps": reading.steps,
            "ready": preview.as_ref().is_some_and(|p| p.problems.is_empty()),
            "preview": preview.as_ref().map(|p| json!({
                "problems": p.problems,
                "before": summary(project),
                "after": p.problems.is_empty().then(|| summary(&p.project)),
            })),
        });
        println!("{out:#}");
    } else {
        print!("{}", describe(&reading));
        if let Some(p) = &preview {
            print!("{}", describe_preview(project, p));
        }
    }

    let Some(p) = preview.filter(|p| p.problems.is_empty()) else {
        return Ok(());
    };
    if write {
        let text = p.project.to_toml().map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| format!("can't write {}: {e}", path.display()))?;
        if !as_json {
            println!("Saved {}.", path.display());
        }
    } else if !as_json {
        println!("Run again with --write to save it.");
    }
    Ok(())
}

fn parse_answers(raw: &[String]) -> Result<Answers, String> {
    raw.iter()
        .map(|a| {
            a.split_once('=')
                .map(|(k, v)| (k.trim().to_string(), Answer::from_user(v.trim())))
                .ok_or_else(|| format!("{a:?} isn't an answer; write it as ID=LABEL"))
        })
        .collect()
}

struct Preview {
    project: Project,
    problems: Vec<String>,
}

fn preview(project: &Project, edits: &[sticks_intent::Edit]) -> Result<Preview, String> {
    let changed = apply_all(project, edits).map_err(|e| e.to_string())?;
    let problems = framing_problems(&changed);
    Ok(Preview {
        project: changed,
        problems,
    })
}

fn summary(project: &Project) -> serde_json::Value {
    match crate::plan(project) {
        Ok(cl) => {
            let s = cl.summary();
            json!({ "boards": s.boards, "sheets": s.sheets, "pieces": s.pieces, "total_cents": s.total_cents })
        }
        Err(e) => json!({ "error": e }),
    }
}

fn describe(reading: &Interpretation) -> String {
    let mut out = String::new();
    for step in &reading.steps {
        out += &format!("\"{}\"\n", step.text);
        for e in &step.edits {
            out += &format!("  {e}\n");
        }
        for a in &step.assumed {
            out += &format!("    assumed: {a}\n");
        }
        if let Some(q) = &step.question {
            out += &format!("  ? {} ({})\n", q.prompt, q.reason);
            for c in &q.options {
                out += &format!("      --answer {}={}   {}\n", q.id, c.label, c.description);
            }
        }
        if let Some(p) = &step.problem {
            out += &format!("  ! {p}\n");
        }
    }
    out
}

fn describe_preview(before: &Project, p: &Preview) -> String {
    if !p.problems.is_empty() {
        let mut out = "This won't frame:\n".to_string();
        for e in &p.problems {
            out += &format!("  ! {e}\n");
        }
        return out;
    }
    match (crate::plan(before), crate::plan(&p.project)) {
        (Ok(a), Ok(b)) => {
            let (a, b) = (a.summary(), b.summary());
            let cost = match (a.total_cents, b.total_cents) {
                (Some(x), Some(y)) => format!(", cost {} -> {}", dollars(x), dollars(y)),
                _ => String::new(),
            };
            format!(
                "It frames. Boards {} -> {}, sheets {} -> {}{cost}.\n",
                a.boards, b.boards, a.sheets, b.sheets
            )
        }
        (_, Err(e)) => format!("It frames, but the cut list fails: {e}\n"),
        (Err(_), Ok(_)) => "It frames.\n".to_string(),
    }
}

fn dollars(cents: u64) -> String {
    format!("${}.{:02}", cents / 100, cents % 100)
}
