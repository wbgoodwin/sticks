//! Runs the Jev probe's sample requests through the local model instead, in
//! the same layout, so the two can be read side by side: what each part of a
//! request changes, the picks, the edits, and what's left for you.
//!
//!   cd app && npm run probe:local                 (the default model)
//!   STICKS_MODEL=path/to/model.gguf npm run probe:local
//!   npm run probe:local -- "add a window"        (your own requests)

use std::path::Path;
use std::time::Instant;

use sticks_core::Project;
use sticks_intent::{interpret, Answers, CONFIDENT};
use sticks_local::{find_model, settle, LocalModel, Odds};

/// The same list as app/scripts/jev-probe.ts.
const REQUESTS: &[&str] = &[
    "make it 20' wide and add a window on the left wall",
    "beef up the rafters to 2x8",
    "make the roof a gable",
    "let some light into the right wall",
    "I want a way in on the back",
    "get rid of the hole for the mini split",
    "make it bigger by 2'",
    "add a window",
    "the roof should shed rain better",
    "paint it red",
    "put a 36x48 window in the right wall 2' from the left",
    "make the porch 8' deep",
    "please add a window on each side of the door",
    "make the door 60 inches by 80 inches",
    "center the front door",
    "add two windows to the back wall",
];

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let shed = root.join("examples/shed.toml");
    let project = Project::from_toml(&std::fs::read_to_string(&shed).expect("examples/shed.toml"))
        .expect("examples/shed.toml loads");
    let path = find_model(&[&root.join("models")]).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });
    let started = Instant::now();
    let mut model = LocalModel::load(&path).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });
    println!(
        "{} loaded in {:.1}s",
        path.display(),
        started.elapsed().as_secs_f64()
    );

    let args: Vec<String> = std::env::args().skip(1).collect();
    let requests: Vec<&str> = if args.is_empty() {
        REQUESTS.to_vec()
    } else {
        args.iter().map(String::as_str).collect()
    };
    for request in requests {
        let started = Instant::now();
        let mut answers = Answers::new();
        let odds = match settle(&mut model, &project, request, &mut answers) {
            Ok(o) => o,
            Err(e) => {
                println!("\n\"{request}\"  [failed: {e}]");
                continue;
            }
        };
        let took = started.elapsed().as_secs_f64();
        let reading = interpret(&project, request, &answers);
        let asked = reading.steps.iter().any(|s| s.question.is_some());
        let state = if asked { "asks you" } else { "settled" };
        println!("\n\"{request}\"  [{state}, {took:.1}s]");

        // Under 80%, also the runner-up.
        let pct = |id: &str| -> String {
            let Some((_, Odds(o))) = odds.iter().rev().find(|(i, _)| i == id) else {
                return String::new();
            };
            let mut text = format!("{} {:.0}%", o[0].0, o[0].1 * 100.0);
            if o[0].1 < CONFIDENT {
                if let Some((label, p)) = o.get(1) {
                    text += &format!(" (next {label} {:.0}%)", p * 100.0);
                }
            }
            text
        };
        for (i, s) in reading.steps.iter().enumerate() {
            let target = format!("{i}.target");
            if answers.contains_key(&target) {
                println!("  \"{}\": {}", s.text, pct(&target));
            }
            let picks: Vec<String> = answers
                .iter()
                .filter(|(id, a)| {
                    id.starts_with(&format!("{i}.")) && **id != target && a.pick != "unsaid"
                })
                .map(|(id, _)| format!("{} {}", &id[id.find('.').unwrap() + 1..], pct(id)))
                .collect();
            if !picks.is_empty() {
                println!("    picks: {}", picks.join(", "));
            }
            for e in &s.edits {
                println!("    edit: {e}");
            }
            for a in &s.assumed {
                println!("    assumed: {a}");
            }
            if let Some(q) = &s.question {
                println!("    ask: {} ({})", q.prompt, q.reason);
            }
            if let Some(p) = &s.problem {
                println!("    needs: {p}");
            }
        }
        if let Some(edits) = reading.edits() {
            match sticks_intent::apply_all(&project, &edits) {
                Ok(p) => {
                    for problem in sticks_intent::framing_problems(&p) {
                        println!("  won't frame: {problem}");
                    }
                }
                Err(e) => println!("  won't frame: {e}"),
            }
        }
    }
}
