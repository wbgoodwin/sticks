//! Requests against the reference shed, and what they come to.

use sticks_core::Project;
use sticks_intent::{apply_all, framing_problems, interpret, Answers, Interpretation};

fn shed() -> Project {
    Project::from_toml(include_str!("../../../examples/shed.toml")).unwrap()
}

fn read(request: &str, answers: &[(&str, &str)]) -> Interpretation {
    let answers: Answers = answers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    interpret(&shed(), request, &answers)
}

/// The edits in builder English, or panics with what stopped them.
fn edits(request: &str) -> Vec<String> {
    let r = read(request, &[]);
    match r.edits() {
        Some(e) => e.iter().map(|e| e.to_string()).collect(),
        None => panic!("{request:?} didn't settle: {r:#?}"),
    }
}

#[test]
fn settles_plain_requests_without_asking() {
    let cases: &[(&str, &[&str])] = &[
        ("make it 20' wide", &["Make the building 20' wide"]),
        ("Make it 20 feet wide and 16 ft deep", &["Make the building 20' wide", "Make the building 16' deep"]),
        ("make it 2' wider", &["Make the building 20' wide"]),
        ("make the shed 12x16", &["Make the building 12' wide", "Make the building 16' deep"]),
        ("make it 20 by 16", &["Make the building 20' wide", "Make the building 16' deep"]),
        ("raise the front wall to 10' 6\"", &["Make the building 10' 6\" tall at the front"]),
        ("a 3 in 12 roof", &["Set the roof pitch to 3:12"]),
        ("steeper roof", &["Set the roof pitch to 3:12"]),
        ("make the roof a gable at 6/12", &["Make the roof a gable", "Set the roof pitch to 6:12"]),
        ("18\" overhangs", &["Set the front and back overhangs to 18\""]),
        ("make the porch 8' deep", &["Make the porch 8' deep"]),
        ("no porch", &["Remove the porch"]),
        ("lose the deck", &["Take the deck out from under the porch"]),
        ("24\" on center", &["Space studs, joists and rafters 24\" on center"]),
        ("use 2x6 studs", &["Use 2x6 studs"]),
        ("2x8 rafters", &["Use 2x8 rafters"]),
        ("remove the door", &["Remove opening 1 from the front wall"]),
        (
            "add a 3' window on the left wall",
            &["Add a 3' x 3' window to the left wall, 5' 6\" from its left end"],
        ),
        (
            "put a 36x48 window in the right wall 2' from the left",
            &["Add a 3' x 4' window to the right wall, 2' from its left end"],
        ),
        (
            "add a 3 ft door to the left wall",
            &["Add a 3' 2-1/2\" x 6' 10-1/2\" door to the left wall, 5' 4-3/4\" from its left end"],
        ),
    ];
    for (request, want) in cases {
        assert_eq!(edits(request), *want, "{request:?}");
    }
}

#[test]
fn asks_with_options_from_the_design() {
    let r = read("add a window", &[]);
    let q = r.questions().next().expect("a question");
    assert_eq!(q.id, "0.wall");
    let labels: Vec<&str> = q.options.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, ["front", "back", "left", "right"]);
    assert!(q.options[0].description.contains("has door, penetration"));

    let r = read("add a window", &[("0.wall", "back")]);
    assert_eq!(
        r.edits().unwrap()[0].to_string(),
        "Add a 3' x 3' window to the back wall, 7' 6\" from its left end"
    );
}

#[test]
fn asks_which_size_when_the_words_dont_say() {
    let r = read("make it bigger", &[]);
    assert_eq!(r.questions().next().unwrap().id, "0.dimension");
    let r = read("make it bigger", &[("0.dimension", "depth")]);
    assert_eq!(
        r.edits().unwrap()[0].to_string(),
        "Make the building 16' deep"
    );
    assert_eq!(r.steps[0].assumed, ["No amount given, so it moves by 2'"]);
}

#[test]
fn ignores_answers_that_arent_options() {
    let r = read("add a window", &[("0.wall", "roof")]);
    assert!(r.edits().is_none());
}

#[test]
fn says_when_a_request_isnt_about_framing() {
    let r = read("paint it red", &[]);
    let q = r.questions().next().unwrap();
    assert!(q.options.iter().any(|c| c.label == "not_framing"));
    let r = read("paint it red", &[("0.intent", "not_framing")]);
    assert!(r.steps[0].problem.is_some());
}

#[test]
fn asks_the_user_for_numbers_it_cant_guess() {
    let r = read("change the spacing", &[]);
    assert!(r.steps[0]
        .problem
        .as_deref()
        .unwrap()
        .contains("24\" on center"));
}

#[test]
fn the_framing_rules_check_the_result() {
    let p = shed();
    let e = read("make it 12x16", &[]).edits().unwrap();
    let problems = framing_problems(&apply_all(&p, &e).unwrap());
    assert!(
        problems[0].contains("doesn't fit inside the wall"),
        "{problems:?}"
    );

    let e = read("add a window on the left wall", &[]).edits().unwrap();
    assert!(framing_problems(&apply_all(&p, &e).unwrap()).is_empty());
}

#[test]
fn edits_round_trip_as_json() {
    let e = read("put a 36x48 window in the right wall", &[])
        .edits()
        .unwrap();
    let text = serde_json::to_string(&e).unwrap();
    assert!(text.contains("\"op\":\"add_opening\""), "{text}");
    let back: Vec<sticks_intent::Edit> = serde_json::from_str(&text).unwrap();
    assert_eq!(back, e);
}

#[test]
fn leaves_questions_the_request_cant_answer_to_the_user() {
    let q = read("add a window", &[])
        .questions()
        .next()
        .cloned()
        .unwrap();
    assert!(q.for_user, "nothing in \"add a window\" names a wall");
    let q = read("let some light into the right wall", &[])
        .questions()
        .next()
        .cloned()
        .unwrap();
    assert!(!q.for_user, "\"light\" is something a classifier can read");
}

#[test]
fn asks_which_way_when_only_the_goal_is_given() {
    let r = read("the roof should shed rain better", &[("0.intent", "pitch")]);
    let q = r.questions().next().unwrap();
    assert_eq!(q.id, "0.direction");
    let labels: Vec<&str> = q.options.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, ["steeper", "flatter"]);
    let r = read(
        "the roof should shed rain better",
        &[("0.intent", "pitch"), ("0.direction", "steeper")],
    );
    assert_eq!(
        r.edits().unwrap()[0].to_string(),
        "Set the roof pitch to 3:12"
    );
}

#[test]
fn slides_a_new_opening_clear_of_the_others() {
    let r = read("add a window on the front wall", &[]);
    let e = r.edits().unwrap();
    assert_eq!(
        e[0].to_string(),
        "Add a 3' x 3' window to the front wall, 11' from its left end"
    );
    assert!(r.steps[0]
        .assumed
        .iter()
        .any(|a| a.starts_with("Moved 3' 6\" right")));
    assert!(framing_problems(&apply_all(&shed(), &e).unwrap()).is_empty());
}

#[test]
fn drops_a_header_size_to_fit_a_door_under_a_low_wall() {
    let r = read("add a door on the back wall", &[]);
    let e = r.edits().unwrap();
    assert!(
        matches!(&e[0], sticks_intent::Edit::AddOpening { header, .. } if header.to_string() == "2x6")
    );
    assert!(framing_problems(&apply_all(&shed(), &e).unwrap()).is_empty());
}

#[test]
fn says_why_an_opening_wont_fit_anywhere() {
    let r = read("add a 10' x 7' door on the back wall", &[]);
    let p = r.steps[0].problem.as_deref().expect("a problem");
    assert!(
        p.starts_with("A 10' x 7' door won't fit in the back wall"),
        "{p}"
    );
}
