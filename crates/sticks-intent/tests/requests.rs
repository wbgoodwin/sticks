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
            &[("0.intent", "resize"), ("0.dimension", "width"), ("0.num0", "size")],
            &["Make the building 20' wide"],
        ),
        (
            "make it 2' wider",
            &[
                ("0.intent", "resize"),
                ("0.dimension", "width"),
                ("0.num0", "change"),
                ("0.direction", "increase"),
            ],
            &["Make the building 20' wide"],
        ),
        (
            "make the shed 12x16",
            &[("0.intent", "resize")],
            &["Make the building 12' wide", "Make the building 16' deep"],
        ),
        (
            "a 3 in 12 roof",
            &[("0.intent", "pitch")],
            &["Set the roof pitch to 3:12"],
        ),
        (
            "the roof should shed rain better",
            &[("0.intent", "pitch"), ("0.direction", "increase")],
            &["Set the roof pitch to 3:12"],
        ),
        (
            "make the roof a gable at 6/12",
            &[("0.intent", "roof_kind"), ("0.roof_kind", "gable")],
            &["Make the roof a gable", "Set the roof pitch to 6:12"],
        ),
        (
            "18\" overhangs",
            &[("0.intent", "overhang"), ("0.num0", "size")],
            &["Set the front and back overhangs to 18\""],
        ),
        (
            "make the porch 8' deep",
            &[("0.intent", "porch"), ("0.porch", "add_or_resize"), ("0.num0", "size")],
            &["Make the porch 8' deep"],
        ),
        (
            "lose the porch",
            &[("0.intent", "porch"), ("0.porch", "remove")],
            &["Remove the porch"],
        ),
        (
            "24\" on center",
            &[("0.intent", "spacing"), ("0.num0", "size")],
            &["Space studs, joists and rafters 24\" on center"],
        ),
        (
            "beef up the rafters to 2x8",
            &[("0.intent", "lumber"), ("0.framing", "rafters")],
            &["Use 2x8 rafters"],
        ),
        (
            "get rid of the hole for the mini split",
            &[("0.intent", "remove_opening"), ("0.which", "front-2")],
            &["Remove opening 2 from the front wall"],
        ),
        (
            "put a 36x48 window in the right wall 2' from the left",
            &[
                ("0.intent", "add_opening"),
                ("0.kind", "window"),
                ("0.wall", "right"),
                ("0.num0", "from_left"),
            ],
            &["Add a 3' x 4' window to the right wall, 2' from its left end"],
        ),
        (
            "add a 3 ft door to the left wall",
            &[
                ("0.intent", "add_opening"),
                ("0.kind", "door"),
                ("0.wall", "left"),
                ("0.num0", "width"),
            ],
            &["Add a 3' 2-1/2\" x 6' 10-1/2\" door to the left wall, 5' 4-3/4\" from its left end"],
        ),
    ];
    for (request, picks, want) in cases {
        assert_eq!(edits(request, picks), *want, "{request:?}");
    }
}

#[test]
fn reads_each_clause_on_its_own() {
    let got = edits(
        "make it 20' wide and add a window on the left wall",
        &[
            ("0.intent", "resize"),
            ("0.dimension", "width"),
            ("0.num0", "size"),
            ("1.intent", "add_opening"),
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

#[test]
fn asks_one_batch_of_questions_built_from_the_design() {
    let asks = questions(&shed(), "put a window 2' from the left");
    let ids: Vec<&str> = asks
        .iter()
        .map(|a| match a {
            Ask::Choice { id, .. } => id.as_str(),
        })
        .collect();
    for want in ["0.intent", "0.wall", "0.kind", "0.which", "0.num0"] {
        assert!(ids.contains(&want), "{want} missing from {ids:?}");
    }
    let wall = asks.iter().find_map(|a| match a {
        Ask::Choice { id, options, .. } if id == "0.wall" => Some(options),
        _ => None,
    });
    let wall = wall.unwrap();
    assert!(wall[0].description.contains("has door, penetration"));
    assert_eq!(wall.last().unwrap().label, "unsaid");
    let num = asks.iter().find_map(|a| match a {
        Ask::Choice { id, prompt, .. } if id == "0.num0" => Some(prompt.as_str()),
        _ => None,
    });
    assert_eq!(num, Some("In this request, what is 2' for?"));
}

#[test]
fn asks_the_user_when_the_classifier_isnt_sure() {
    let mut answers = sure(&[
        ("0.intent", "resize"),
        ("0.num0", "change"),
        ("0.direction", "increase"),
    ]);
    answers.insert(
        "0.dimension".into(),
        Answer {
            pick: "width".into(),
            confidence: 0.64,
            by_user: false,
        },
    );
    let r = read("make it bigger by 2'", &answers);
    let q = r.questions().next().unwrap();
    assert_eq!(q.id, "0.dimension");
    assert_eq!(q.reason, "Best guess width at 64%");

    answers.insert("0.dimension".into(), Answer::from_user("depth"));
    let r = read("make it bigger by 2'", &answers);
    assert_eq!(
        r.edits().unwrap()[0].to_string(),
        "Make the building 16' deep"
    );
}

#[test]
fn asks_the_user_when_the_request_doesnt_say() {
    let mut answers = sure(&[
        ("0.intent", "add_opening"),
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
        ("0.intent", "Not answered yet")
    );
}

#[test]
fn ignores_answers_that_arent_options() {
    let r = read(
        "add a window",
        &sure(&[
            ("0.intent", "add_opening"),
            ("0.kind", "window"),
            ("0.wall", "roof"),
        ]),
    );
    assert_eq!(r.questions().next().unwrap().id, "0.wall");
}

#[test]
fn says_when_a_request_isnt_about_framing() {
    let r = read("paint it red", &sure(&[("0.intent", "not_framing")]));
    assert!(r.steps[0].problem.is_some());
}

#[test]
fn asks_the_user_for_numbers_it_cant_guess() {
    let r = read("change the spacing", &sure(&[("0.intent", "spacing")]));
    assert!(r.steps[0]
        .problem
        .as_deref()
        .unwrap()
        .contains("24\" on center"));
}

#[test]
fn slides_a_new_opening_clear_of_the_others() {
    let picks = [
        ("0.intent", "add_opening"),
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
        ("0.intent", "add_opening"),
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
fn puts_openings_beside_an_existing_one() {
    let picks = [
        ("0.intent", "add_opening"),
        ("0.kind", "window"),
        ("0.position", "both_sides"),
        ("0.next_to", "front-1"),
    ];
    let r = read(
        "please add a window on each side of the door",
        &sure(&picks),
    );
    let e = r.edits().unwrap();
    assert_eq!(
        e.iter().map(|e| e.to_string()).collect::<Vec<_>>(),
        [
            "Add a 3' x 3' window to the front wall, 3' 3\" from its left end",
            "Add a 3' x 3' window to the front wall, 11' 5-1/2\" from its left end",
        ]
    );
    assert!(framing_problems(&apply_all(&shed(), &e).unwrap()).is_empty());

    // Only one side, and the request doesn't say which wall: the door's.
    let picks = [
        ("0.intent", "add_opening"),
        ("0.kind", "window"),
        ("0.wall", "unsaid"),
        ("0.position", "right_of"),
        ("0.next_to", "front-1"),
    ];
    let e = read("a window right of the door", &sure(&picks))
        .edits()
        .unwrap();
    assert_eq!(e.len(), 1);
    assert!(
        matches!(&e[0], Edit::AddOpening { offset, .. } if offset.to_string() == "11' 5-1/2\"")
    );
}

#[test]
fn asks_which_opening_when_beside_one_is_unclear() {
    let picks = [
        ("0.intent", "add_opening"),
        ("0.kind", "window"),
        ("0.position", "left_of"),
        ("0.next_to", "unsaid"),
    ];
    let r = read("put a window next to it", &sure(&picks));
    assert_eq!(r.questions().next().unwrap().id, "0.next_to");
}

#[test]
fn says_why_an_opening_wont_fit_anywhere() {
    let picks = [
        ("0.intent", "add_opening"),
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
    let e = read("make it 12x16", &sure(&[("0.intent", "resize")]))
        .edits()
        .unwrap();
    let problems = framing_problems(&apply_all(&shed(), &e).unwrap());
    assert!(
        problems[0].contains("doesn't fit inside the wall"),
        "{problems:?}"
    );
}

#[test]
fn edits_round_trip_as_json() {
    let picks = [
        ("0.intent", "add_opening"),
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
