//! Runs only with the `llama` feature and a model in STICKS_TEST_MODEL; any
//! GGUF with a ChatML vocabulary will do, even one with random weights.
#![cfg(feature = "llama")]

use std::path::PathBuf;

use sticks_core::Project;
use sticks_intent::Answers;
use sticks_local::{Answerer, LocalModel, Query};

fn model() -> Option<LocalModel> {
    let path = PathBuf::from(std::env::var_os("STICKS_TEST_MODEL")?);
    Some(LocalModel::load(&path).unwrap())
}

fn shed() -> Project {
    Project::from_toml(include_str!("../../../examples/shed.toml")).unwrap()
}

#[test]
fn each_question_reads_as_if_asked_alone() {
    let Some(mut m) = model() else { return };
    let project = shed();
    let request = "make it 20' wide and add a window on the left wall";
    let design = serde_json::to_string_pretty(&sticks_intent::design(&project)).unwrap();
    let queries: Vec<Query> = sticks_intent::questions(&project, request, &Answers::new())
        .iter()
        .map(Query::from)
        .collect();
    assert_eq!(queries.len(), 2);
    let both = m.odds(request, &design, &queries).unwrap();
    let alone = m.odds(request, &design, &queries[1..]).unwrap();
    // The first question's tokens were cleared before the second was read.
    for ((label, a), (_, b)) in both[1].1 .0.iter().zip(&alone[0].1 .0) {
        assert!((a - b).abs() < 1e-3, "{label}: {a} vs {b}");
    }
}

#[test]
fn picks_only_listed_options() {
    let Some(mut m) = model() else { return };
    let project = shed();
    let request = "beef up the rafters to 2x8";
    let design = serde_json::to_string_pretty(&sticks_intent::design(&project)).unwrap();
    let queries: Vec<Query> = sticks_intent::questions(&project, request, &Answers::new())
        .iter()
        .map(Query::from)
        .collect();
    let answers = m.answer(request, &design, &queries).unwrap();
    let a = &answers["0.target"];
    assert!(queries[0].options.iter().any(|o| o.label == a.pick));
    assert!(a.confidence > 0.0 && a.confidence <= 1.0);
}
