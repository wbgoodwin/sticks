//! Requests against the reference shed, with a classifier's answers given by
//! hand, and what they come to.

use sticks_core::Project;
use sticks_intent::{
    apply_all, framing_problems, interpret, questions, Answer, Answers, Ask, Edit, Interpretation,
};

fn shed() -> Project {
    Project::from_toml(include_str!("../../../examples/shed.toml")).unwrap()
}

/// Confident classifier answers.
fn sure(picks: &[(&str, &str)]) -> Answers {
    picks
        .iter()
        .map(|(id, pick)| {
            let a = Answer {
                pick: pick.to_string(),
                confidence: 0.95,
                by_user: false,
            };
            (id.to_string(), a)
        })
        .collect()
}

fn read(request: &str, answers: &Answers) -> Interpretation {
    interpret(&shed(), request, answers)
}

/// The edits in builder English, or panics with what stopped them.
fn edits(request: &str, picks: &[(&str, &str)]) -> Vec<String> {
    let r = read(request, &sure(picks));
    match r.edits() {
        Some(e) => e.iter().map(|e| e.to_string()).collect(),
        None => panic!("{request:?} didn't settle: {r:#?}"),
    }
}

/// A request, the classifier's picks, and the edits they come to.
type Case<'a> = (&'a str, &'a [(&'a str, &'a str)], &'a [&'a str]);

#[test]
fn turns_confident_answers_into_edits() {
    let cases: &[Case] = &[
        (
            "make it 20' wide",
            &[("0.target", "width"), ("0.num0", "new")],
            &["Make the building 20' wide"],
        ),
        (
            "make it 2' wider",
            &[
                ("0.target", "width"),
                ("0.how", "increase"),
                ("0.num0", "amount"),
            ],
            &["Make the building 20' wide"],
        ),
        (
            "make the shed 12x16",
            &[("0.target", "footprint")],
            &["Make the building 12' wide", "Make the building 16' deep"],
        ),
        (
            "a 3 in 12 roof",
            &[("0.target", "pitch")],
            &["Set the roof pitch to 3:12"],
        ),
        (
            "the roof should shed rain better",
            &[("0.target", "pitch"), ("0.how", "increase")],
            &["Set the roof pitch to 3:12"],
        ),
        (
            "make the roof a gable at 6/12",
            &[("0.target", "roof_shape"), ("0.shape", "gable")],
            &["Make the roof a gable", "Set the roof pitch to 6:12"],
        ),
        (
            "18\" overhangs",
            &[("0.target", "overhangs"), ("0.num0", "new")],
            &["Set the front and back overhangs to 18\""],
        ),
        (
            "make the porch 8' deep",
            &[("0.target", "porch_depth"), ("0.num0", "new")],
            &["Make the porch 8' deep"],
        ),
        (
            "lose the porch",
            &[("0.target", "porch"), ("0.porch", "remove")],
            &["Remove the porch"],
        ),
        (
            "24\" on center",
            &[("0.target", "spacing"), ("0.num0", "new")],
            &["Space studs, joists and rafters 24\" on center"],
        ),
        (
            "beef up the rafters to 2x8",
            &[("0.target", "rafters"), ("0.how", "increase")],
            &["Use 2x8 rafters"],
        ),
        (
            "beef up the rafters",
            &[("0.target", "rafters"), ("0.how", "increase")],
            &["Use 2x8 rafters"],
        ),
        (
            "get rid of the hole for the mini split",
            &[("0.target", "front-2"), ("0.action", "remove")],
            &["Remove opening 2 from the front wall"],
        ),
        (
            "put a 36x48 window in the right wall 2' from the left",
            &[
                ("0.target", "new_opening"),
                ("0.kind", "window"),
                ("0.wall", "right"),
                ("0.place", "left_end"),
                ("0.num0", "distance"),
            ],
            &["Add a 3' x 4' window to the right wall, 2' from its left end"],
        ),
        (
            "add a 3 ft door to the left wall",
            &[
                ("0.target", "new_opening"),
                ("0.kind", "door"),
                ("0.wall", "left"),
                ("0.num0", "width"),
            ],
            &["Add a 3' 2-1/2\" x 6' 10-1/2\" door to the left wall, 5' 4-3/4\" from its left end"],
        ),
        (
            "center the front door",
            &[
                ("0.target", "front-1"),
                ("0.action", "move"),
                ("0.place", "center"),
            ],
            &["Make the front wall's door 3' 2-1/2\" x 6' 10-1/2\", 7' 4-3/4\" from its left end"],
        ),
        (
            "make the door 60 inches by 80 inches",
            &[("0.target", "front-1"), ("0.action", "resize")],
            &["Make the front wall's door 5' x 6' 8\", 6' 4-1/4\" from its left end"],
        ),
        (
            "please add a window on each side of the door",
            &[
                ("0.target", "new_opening"),
                ("0.kind", "window"),
                ("0.place", "both_sides"),
                ("0.ref", "front-1"),
            ],
            &[
                "Add a 3' x 3' window to the front wall, 3' 3\" from its left end",
                "Add a 3' x 3' window to the front wall, 11' 5-1/2\" from its left end",
            ],
        ),
        (
            "add two windows to the back wall",
            &[
                ("0.target", "new_opening"),
                ("0.kind", "window"),
                ("0.wall", "back"),
                ("0.count", "two"),
            ],
            &[
                "Add a 3' x 3' window to the back wall, 4' 6\" from its left end",
                "Add a 3' x 3' window to the back wall, 10' 6\" from its left end",
            ],
        ),
    ];
    for (request, picks, want) in cases {
        let got = edits(request, picks);
        assert_eq!(got, *want, "{request:?}");
        let e = read(request, &sure(picks)).edits().unwrap();
        assert!(
            framing_problems(&apply_all(&shed(), &e).unwrap()).is_empty()
                || *request == "make the shed 12x16",
            "{request:?} doesn't frame"
        );
    }
}

#[test]
fn reads_each_clause_on_its_own() {
    let got = edits(
        "make it 20' wide and add a window on the left wall",
        &[
            ("0.target", "width"),
            ("0.num0", "new"),
            ("1.target", "new_opening"),
            ("1.kind", "window"),
            ("1.wall", "left"),
        ],
    );
    assert_eq!(
        got,
        [
            "Make the building 20' wide",
            "Add a 3' x 3' window to the left wall, 5' 6\" from its left end"
        ]
    );
}

fn ids(asks: &[Ask]) -> Vec<&str> {
    asks.iter()
        .map(|a| match a {
            Ask::Choice { id, .. } => id.as_str(),
        })
        .collect()
}

#[test]
fn asks_the_target_first_then_its_own_questions() {
    let request = "put a window 2' from the left";
    let first = questions(&shed(), request, &Answers::new());
    assert_eq!(ids(&first), ["0.target"]);
    let Ask::Choice { options, .. } = &first[0];
    let door = options.iter().find(|o| o.label == "front-1").unwrap();
    assert_eq!(
        door.description,
        "The only door, in the front wall, 3' 2-1/2\" x 6' 10-1/2\", 7' 3\" from its left end"
    );
    assert!(options.iter().any(|o| o.label == "new_opening"));

    let answers = sure(&[("0.target", "new_opening")]);
    let second = questions(&shed(), request, &answers);
    for want in ["0.kind", "0.wall", "0.count", "0.place", "0.ref", "0.num0"] {
        assert!(
            ids(&second).contains(&want),
            "{want} missing from {:?}",
            ids(&second)
        );
    }
    let wall = second.iter().find_map(|a| match a {
        Ask::Choice { id, options, .. } if id == "0.wall" => Some(options),
        _ => None,
    });
    let wall = wall.unwrap();
    assert!(wall[0].description.contains("has door, penetration"));
    assert_eq!(wall.last().unwrap().label, "unsaid");
    let num = second.iter().find_map(|a| match a {
        Ask::Choice { id, prompt, .. } if id == "0.num0" => Some(prompt.as_str()),
        _ => None,
    });
    assert_eq!(num, Some("In this request, what is 2' for?"));

    // Answered: nothing left for the classifier.
    let mut all = answers.clone();
    for id in ids(&second) {
        all.insert(id.to_string(), Answer::from_user("unsaid"));
    }
    assert!(questions(&shed(), request, &all).is_empty());
}

#[test]
fn leaves_an_unsure_target_to_the_user() {
    let mut answers = Answers::new();
    answers.insert(
        "0.target".into(),
        Answer {
            pick: "width".into(),
            confidence: 0.64,
            by_user: false,
        },
    );
    let r = read("make it bigger by 2'", &answers);
    let q = r.questions().next().unwrap();
    assert_eq!(q.id, "0.target");
    assert_eq!(q.reason, "Best guess width at 64%");
    // No more questions for the classifier until the user picks.
    assert!(questions(&shed(), "make it bigger by 2'", &answers).is_empty());

    answers.insert("0.target".into(), Answer::from_user("depth"));
    assert_eq!(
        ids(&questions(&shed(), "make it bigger by 2'", &answers)),
        ["0.how", "0.num0"]
    );
    answers.extend(sure(&[("0.how", "increase"), ("0.num0", "amount")]));
    let r = read("make it bigger by 2'", &answers);
    assert_eq!(
        r.edits().unwrap()[0].to_string(),
        "Make the building 16' deep"
    );
}

#[test]
fn asks_the_user_when_the_request_doesnt_say() {
    let mut answers = sure(&[
        ("0.target", "new_opening"),
        ("0.kind", "window"),
        ("0.wall", "unsaid"),
    ]);
    let r = read("add a window", &answers);
    let q = r.questions().next().unwrap();
    assert_eq!(
        (q.id.as_str(), q.reason.as_str()),
        ("0.wall", "The request doesn't say")
    );

    answers.insert("0.wall".into(), Answer::from_user("back"));
    let r = read("add a window", &answers);
    assert_eq!(
        r.edits().unwrap()[0].to_string(),
        "Add a 3' x 3' window to the back wall, 7' 6\" from its left end"
    );
}

#[test]
fn waits_for_answers_before_doing_anything() {
    let r = read("make it 20' wide", &Answers::new());
    let q = r.questions().next().unwrap();
    assert_eq!(
        (q.id.as_str(), q.reason.as_str()),
        ("0.target", "Not answered yet")
    );
}

#[test]
fn ignores_answers_that_arent_options() {
    let r = read(
        "add a window",
        &sure(&[
            ("0.target", "new_opening"),
            ("0.kind", "window"),
            ("0.wall", "roof"),
        ]),
    );
    assert_eq!(r.questions().next().unwrap().id, "0.wall");
}

#[test]
fn says_when_a_request_is_about_something_else() {
    let r = read("paint it red", &sure(&[("0.target", "something_else")]));
    assert!(r.steps[0].problem.is_some());
}

#[test]
fn asks_for_numbers_it_cant_guess() {
    let mut answers = sure(&[("0.target", "spacing"), ("0.how", "unsaid")]);
    let r = read("change the spacing", &answers);
    assert_eq!(r.questions().next().unwrap().id, "0.how");
    answers.insert("0.how".into(), Answer::from_user("increase"));
    let r = read("change the spacing", &answers);
    assert_eq!(
        r.edits().unwrap()[0].to_string(),
        "Space studs, joists and rafters 24\" on center"
    );
    let r = read(
        "make the door bigger",
        &sure(&[("0.target", "front-1"), ("0.action", "resize")]),
    );
    assert!(r.steps[0]
        .problem
        .as_deref()
        .unwrap()
        .starts_with("Say the new size"));
}

#[test]
fn slides_a_new_opening_clear_of_the_others() {
    let picks = [
        ("0.target", "new_opening"),
        ("0.kind", "window"),
        ("0.wall", "front"),
    ];
    let r = read("add a window on the front wall", &sure(&picks));
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
    let picks = [
        ("0.target", "new_opening"),
        ("0.kind", "door"),
        ("0.wall", "back"),
    ];
    let e = read("I want a way in on the back", &sure(&picks))
        .edits()
        .unwrap();
    assert!(matches!(&e[0], Edit::AddOpening { header, .. } if header.to_string() == "2x6"));
    assert!(framing_problems(&apply_all(&shed(), &e).unwrap()).is_empty());
}

#[test]
fn says_why_an_opening_wont_fit_anywhere() {
    let picks = [
        ("0.target", "new_opening"),
        ("0.kind", "door"),
        ("0.wall", "back"),
    ];
    let r = read("add a 10' x 7' door on the back wall", &sure(&picks));
    let p = r.steps[0].problem.as_deref().expect("a problem");
    assert!(
        p.starts_with("A 10' x 7' door won't fit in the back wall"),
        "{p}"
    );
}

#[test]
fn the_framing_rules_check_the_result() {
    let e = read("make it 12x16", &sure(&[("0.target", "footprint")]))
        .edits()
        .unwrap();
    let problems = framing_problems(&apply_all(&shed(), &e).unwrap());
    assert!(
        problems[0].contains("doesn't fit inside the wall"),
        "{problems:?}"
    );
}

#[test]
fn says_what_each_step_changes() {
    let r = read(
        "center the front door",
        &sure(&[
            ("0.target", "front-1"),
            ("0.action", "move"),
            ("0.place", "center"),
        ]),
    );
    assert_eq!(r.steps[0].target.as_deref(), Some("front-1"));
}

#[test]
fn edits_round_trip_as_json() {
    let picks = [
        ("0.target", "new_opening"),
        ("0.kind", "window"),
        ("0.wall", "right"),
    ];
    let e = read("put a 36x48 window in the right wall", &sure(&picks))
        .edits()
        .unwrap();
    let text = serde_json::to_string(&e).unwrap();
    assert!(text.contains("\"op\":\"add_opening\""), "{text}");
    let back: Vec<Edit> = serde_json::from_str(&text).unwrap();
    assert_eq!(back, e);
}

#[test]
fn puts_it_beside_the_only_door_or_window_when_unsure_which() {
    let request = "please add a window on each side of the door";
    let picks = [
        ("0.target", "new_opening"),
        ("0.kind", "window"),
        ("0.count", "two"),
        ("0.place", "both_sides"),
        ("0.ref", "unsaid"),
    ];
    let asks = questions(&shed(), request, &sure(&picks[..1]));
    let unsaid = asks.iter().find_map(|a| match a {
        Ask::Choice { id, options, .. } if id == "0.ref" => options.last(),
        _ => None,
    });
    assert_eq!(
        unsaid.unwrap().description,
        "The request names nothing already there to go next to"
    );

    let mut answers = sure(&picks);
    let r = read(request, &answers);
    assert_eq!(r.edits().unwrap().len(), 2);
    assert!(r.steps[0].assumed.contains(
        &"Beside the door in the front wall, the only door or window in the design".into()
    ));

    // A wall with none to go beside: ask.
    answers.extend(sure(&[("0.wall", "back")]));
    let r = read(request, &answers);
    assert_eq!(r.questions().next().unwrap().id, "0.ref");
}

#[test]
fn steps_the_pitch_when_no_amount_is_given() {
    let r = read(
        "the roof should shed rain better",
        &sure(&[("0.target", "pitch"), ("0.how", "increase")]),
    );
    assert_eq!(
        r.steps[0].assumed,
        ["No amount given, so it changes by 1 in 12"]
    );
}

#[test]
fn gives_the_classifier_the_design_with_the_options_labels() {
    let d = serde_json::to_value(sticks_intent::design(&shed())).unwrap();
    let walls: Vec<&str> = d["walls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["id"].as_str().unwrap())
        .collect();
    assert_eq!(walls, ["front", "back", "left", "right"]);
    let door = &d["walls"][0]["openings"][0];
    assert_eq!(
        (&door["id"], &door["kind"]),
        (&"front-1".into(), &"door".into())
    );
    assert_eq!(d["openings"]["doors"], 1);
    assert_eq!(d["roof"]["pitch"], "2:12");
}
