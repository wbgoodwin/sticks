//! Turns a request into edits.
//!
//! Rust reads what's notation: it splits the request into clauses and reads
//! the lengths, pitches and sizes in each. Everything that's meaning is a
//! fixed-option question answered by a classifier such as Jev, built from
//! the design itself. First: what in the design does each clause change?
//! The options are every part of the design that can change, with its
//! value now, plus a new opening. Then, for that target: what to do to it
//! (set it, make it bigger or smaller, center it, put it beside another
//! opening, how many), and what each number is for.
//!
//! [`questions`] lists what the classifier still needs to answer, given the
//! answers so far: the targets first, then each target's own questions, so
//! a request takes two rounds. [`interpret`] turns confident answers into
//! edits; anything the classifier wasn't sure of, or the request doesn't
//! say, comes back as a [`Question`] for the user.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sticks_core::{Length, Nominal, OpeningKind, Pitch, Project, RoofKind};

use crate::edit::{kind_name, Dimension, Edit, Framing, Side};
use crate::words::{clauses, lumber, tokens, Num, Tok, Unit};

/// Below this, a classifier's pick goes to the user to confirm.
pub const CONFIDENT: f64 = 0.8;

/// An answer to a question, by question id.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Answer {
    /// The option label.
    pub pick: String,
    /// From zero to one.
    pub confidence: f64,
    /// The user answered, which settles it.
    #[serde(default)]
    pub by_user: bool,
}

impl Answer {
    pub fn from_user(pick: &str) -> Self {
        Answer {
            pick: pick.to_string(),
            confidence: 1.0,
            by_user: true,
        }
    }
}

pub type Answers = BTreeMap<String, Answer>;

/// A question for the classifier.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Ask {
    /// Pick one option.
    Choice {
        id: String,
        /// The clause it's about.
        text: String,
        prompt: String,
        options: Vec<Choice>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Interpretation {
    pub steps: Vec<Step>,
}

impl Interpretation {
    /// Every edit, in order, once nothing is left to ask or fix.
    pub fn edits(&self) -> Option<Vec<Edit>> {
        self.steps
            .iter()
            .all(|s| s.question.is_none() && s.problem.is_none())
            .then(|| self.steps.iter().flat_map(|s| s.edits.clone()).collect())
    }

    pub fn questions(&self) -> impl Iterator<Item = &Question> {
        self.steps.iter().filter_map(|s| s.question.as_ref())
    }
}

/// One clause of the request and what it came to.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Step {
    pub text: String,
    /// What in the design it changes, once that's settled: a target label
    /// such as `width` or `front-1`, so the app can show it in the panel.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    pub edits: Vec<Edit>,
    /// Choices made without being told, so the preview can show them.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub assumed: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question: Option<Question>,
    /// Something only the user can fix, such as a missing number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
}

/// A question for the user.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Question {
    /// Stable across runs of the same request: `{clause}.{slot}`.
    pub id: String,
    /// The clause it's about.
    pub text: String,
    pub prompt: String,
    pub options: Vec<Choice>,
    /// Why it's asked: the request doesn't say, or the classifier's pick
    /// and how sure it was.
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Choice {
    pub label: String,
    pub description: String,
}

/// Something in the design a clause can change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Size(Dimension),
    /// Width and depth together.
    Footprint,
    Pitch,
    RoofShape,
    Overhangs,
    Lumber(Framing),
    Spacing,
    Porch,
    PorchDepth,
    Deck,
    /// The opening at this index in that wall.
    Opening(Side, usize),
    NewOpening,
}

/// The target option for anything not in the list.
const SOMETHING_ELSE: &str = "something_else";

/// The option for "the request doesn't say".
const UNSAID: &str = "unsaid";

/// Where an opening goes along its wall.
const PLACES: [(&str, &str); 10] = [
    ("center", "Centered on the wall"),
    (
        "left_end",
        "At the wall's left end seen from outside, or a distance from it",
    ),
    (
        "right_end",
        "At the wall's right end seen from outside, or a distance from it",
    ),
    ("spread", "Several spread evenly along the wall"),
    (
        "left_of",
        "Beside another door or window, on its left seen from outside",
    ),
    (
        "right_of",
        "Beside another door or window, on its right seen from outside",
    ),
    ("both_sides", "One on each side of another door or window"),
    ("shift_left", "Moved left from where it is now"),
    ("shift_right", "Moved right from where it is now"),
    ("other", "Somewhere else"),
];

/// Places that only make sense for a new opening, and for one being moved.
const NEW_ONLY: [&str; 2] = ["spread", "both_sides"];
const MOVE_ONLY: [&str; 2] = ["shift_left", "shift_right"];

enum Stop {
    Ask(Question),
    Problem(String),
}

type Out = Result<Vec<Edit>, Stop>;

/// Rough-opening defaults, matching what the app's panel adds.
fn opening_defaults(kind: OpeningKind) -> (Length, Length, Length, Nominal) {
    match kind {
        OpeningKind::Door => (
            Length::from_sixteenths(616),
            Length::from_sixteenths(1320),
            Length::ZERO,
            Nominal::new(2, 8),
        ),
        OpeningKind::Window => (
            Length::inches(36),
            Length::inches(36),
            Length::inches(44),
            Nominal::new(2, 6),
        ),
        OpeningKind::Penetration => (
            Length::inches(4),
            Length::inches(4),
            Length::feet(1),
            Nominal::new(2, 4),
        ),
    }
}

/// Slab widths a door is sold in. "A 3' door" names the slab; the framed
/// opening is 2-1/2" wider.
const SLAB_WIDTHS: [i64; 9] = [24, 28, 30, 32, 34, 36, 42, 48, 72];

/// Dimensional lumber widths, for "bigger" and "smaller".
const WIDTHS: [u8; 5] = [4, 6, 8, 10, 12];

/// The questions a classifier should answer next about `request`, given the
/// answers so far: which target each clause changes, then, once that's
/// settled, the target's own questions. Empty once there's nothing left
/// for the classifier; anything still open is for the user.
pub fn questions(project: &Project, request: &str, answers: &Answers) -> Vec<Ask> {
    clauses(request)
        .into_iter()
        .enumerate()
        .flat_map(|(i, text)| {
            let c = Clause::new(i, text, answers, project);
            let slots = match answers.get(&c.id("target")) {
                None => vec!["target".to_string()],
                Some(_) => match c.settled_target() {
                    Some(t) => c.slots(t),
                    None => Vec::new(),
                },
            };
            slots
                .into_iter()
                .filter(|s| !answers.contains_key(&c.id(s)))
                .map(|slot| {
                    let (prompt, mut options) = c.slot(&slot);
                    if may_be_unsaid(&slot) {
                        options.push((UNSAID.into(), unsaid(&slot).into()));
                    }
                    Ask::Choice {
                        id: c.id(&slot),
                        text: c.text.clone(),
                        prompt,
                        options: options
                            .into_iter()
                            .map(|(label, description)| Choice { label, description })
                            .collect(),
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Slots whose answer the request may simply not give ("add a window":
/// which wall?). Their questions offer [`UNSAID`] so the classifier can say
/// so, rather than being sure of a guess.
fn may_be_unsaid(slot: &str) -> bool {
    !matches!(slot, "target") && !slot.starts_with("num")
}

/// What [`UNSAID`] means for a slot. Saying exactly what's missing keeps
/// "beside the door" from reading as not saying which door.
fn unsaid(slot: &str) -> &'static str {
    match slot {
        "wall" => "The request doesn't name a wall or point to one",
        "ref" => "The request names nothing already there to go next to",
        "place" => "The request doesn't say where along the wall",
        "count" => "The request doesn't say how many",
        "kind" => "The request doesn't say what kind",
        _ => "The request doesn't say",
    }
}

/// The wall for a `wall` slot's label.
fn side_of(label: &str) -> Option<Side> {
    Side::ALL.into_iter().find(|s| s.id() == label)
}

pub fn interpret(project: &Project, request: &str, answers: &Answers) -> Interpretation {
    let steps = clauses(request)
        .into_iter()
        .enumerate()
        .map(|(i, text)| {
            let mut c = Clause::new(i, text.clone(), answers, project);
            let result = c.run();
            let target = c.settled_target().map(|t| c.target_label(t));
            let mut assumed: Vec<String> = Vec::new();
            for a in c.assumed {
                if !assumed.contains(&a) {
                    assumed.push(a);
                }
            }
            let mut step = Step {
                target,
                text,
                edits: Vec::new(),
                assumed,
                question: None,
                problem: None,
            };
            match result {
                Ok(edits) => step.edits = edits,
                Err(Stop::Ask(q)) => step.question = Some(q),
                Err(Stop::Problem(p)) => step.problem = Some(p),
            }
            step
        })
        .collect();
    Interpretation { steps }
}

struct Clause<'a> {
    index: usize,
    text: String,
    toks: Vec<Tok>,
    answers: &'a Answers,
    project: &'a Project,
    assumed: Vec<String>,
}

/// A number's position among the clause's numbers, and the number.
type Numbered<'n> = (usize, &'n Num);

/// The width, height and distance given for an opening.
type OpeningNumbers = (Option<Length>, Option<Length>, Option<Length>);

/// A target, its label and what it is now.
type TargetOption = (Target, String, String);

impl<'a> Clause<'a> {
    fn new(index: usize, text: String, answers: &'a Answers, project: &'a Project) -> Self {
        Clause {
            index,
            toks: tokens(&text),
            text,
            answers,
            project,
            assumed: Vec::new(),
        }
    }

    fn id(&self, slot: &str) -> String {
        format!("{}.{slot}", self.index)
    }

    fn nums(&self) -> Vec<Numbered<'_>> {
        self.toks
            .iter()
            .filter_map(|t| match t {
                Tok::Num(n) => Some(n),
                _ => None,
            })
            .enumerate()
            .collect()
    }

    fn pair(&self) -> Option<(&Num, &Num)> {
        self.toks.iter().find_map(|t| match t {
            Tok::Pair(a, b) => Some((a, b)),
            _ => None,
        })
    }

    fn pitch(&self) -> Option<Pitch> {
        self.toks.iter().find_map(|t| match t {
            Tok::Pitch(p) => Some(*p),
            _ => None,
        })
    }

    /// Everything in this design a clause can change, with its value now.
    fn targets(&self) -> Vec<TargetOption> {
        let p = self.project;
        let mut t: Vec<TargetOption> = Vec::new();
        let mut add = |target, label: &str, description: String| {
            t.push((target, label.to_string(), description))
        };
        if let Some(b) = &p.building {
            add(
                Target::Size(Dimension::Width),
                "width",
                format!(
                    "The building's width, along the front wall, now {}",
                    b.width
                ),
            );
            add(
                Target::Size(Dimension::Depth),
                "depth",
                format!("The building's depth, front to back, now {}", b.depth),
            );
            add(
                Target::Size(Dimension::Height),
                "height",
                format!("The building's height at the front wall, now {}", b.height),
            );
            add(
                Target::Footprint,
                "footprint",
                format!(
                    "The building's width and depth together, now {} x {}",
                    b.width, b.depth
                ),
            );
        }
        if let Some(r) = &p.roof {
            add(
                Target::Pitch,
                "pitch",
                format!("How steep the roof is, now {}", r.pitch),
            );
            let shape = match r.kind {
                RoofKind::LeanTo => "a lean-to (one slope)",
                RoofKind::Gable => "a gable (two slopes)",
            };
            add(
                Target::RoofShape,
                "roof_shape",
                format!("The roof's shape, now {shape}"),
            );
            add(
                Target::Overhangs,
                "overhangs",
                format!(
                    "How far the roof runs past the front and back walls, now {:#}",
                    r.front_overhang
                ),
            );
            add(
                Target::Lumber(Framing::Rafters),
                "rafters",
                format!("The roof rafters, now {}", r.rafter),
            );
        }
        if let Some(f) = &p.floor {
            add(
                Target::Lumber(Framing::Joists),
                "joists",
                format!("The floor joists, now {}", f.joist),
            );
        }
        add(
            Target::Lumber(Framing::Studs),
            "studs",
            format!("The wall studs, now {}", p.rules.wall_lumber),
        );
        add(
            Target::Spacing,
            "spacing",
            format!(
                "How far apart studs, joists and rafters are, now {:#} on center",
                p.rules.on_center
            ),
        );
        if p.building.is_some() {
            match &p.porch {
                Some(porch) => {
                    let deck = if porch.deck.is_some() {
                        " with a deck"
                    } else {
                        ""
                    };
                    add(
                        Target::Porch,
                        "porch",
                        format!("The front porch as a whole, now {} deep{deck}", porch.depth),
                    );
                    add(
                        Target::PorchDepth,
                        "porch_depth",
                        format!("How deep the porch is, now {}", porch.depth),
                    );
                    let now = if porch.deck.is_some() {
                        "there now"
                    } else {
                        "none now"
                    };
                    add(
                        Target::Deck,
                        "deck",
                        format!("The deck under the porch, {now}"),
                    );
                }
                None => add(
                    Target::Porch,
                    "porch",
                    "A front porch, none now".to_string(),
                ),
            }
        }
        for ((side, i), label, description) in self.openings() {
            add(Target::Opening(side, i), &label, description);
        }
        if p.building.is_some() {
            add(
                Target::NewOpening,
                "new_opening",
                "Add a new door, window or hole to a wall".to_string(),
            );
        }
        t
    }

    fn target_label(&self, target: Target) -> String {
        self.targets()
            .into_iter()
            .find(|(t, ..)| *t == target)
            .map(|(_, l, _)| l)
            .unwrap_or_default()
    }

    /// The target once the classifier is sure of it or the user picked it.
    fn settled_target(&self) -> Option<Target> {
        let a = self.answers.get(&self.id("target"))?;
        if !a.by_user && a.confidence < CONFIDENT {
            return None;
        }
        self.targets()
            .into_iter()
            .find(|(_, l, _)| *l == a.pick)
            .map(|(t, ..)| t)
    }

    /// The questions about a settled target, asked in one batch.
    fn slots(&self, target: Target) -> Vec<String> {
        let mut s: Vec<&str> = match target {
            Target::Size(_)
            | Target::Footprint
            | Target::Pitch
            | Target::Overhangs
            | Target::Lumber(_)
            | Target::Spacing
            | Target::PorchDepth => vec!["how"],
            Target::RoofShape => vec!["shape"],
            Target::Porch => vec!["porch", "deck"],
            Target::Deck => vec!["deck"],
            Target::Opening(..) => vec!["action", "place"],
            Target::NewOpening => vec!["kind", "wall", "count", "place"],
        };
        let opening = matches!(target, Target::Opening(..) | Target::NewOpening);
        if opening && self.openings().len() > usize::from(target != Target::NewOpening) {
            s.push("ref");
        }
        let mut s: Vec<String> = s.into_iter().map(str::to_string).collect();
        if !matches!(
            target,
            Target::RoofShape | Target::Porch | Target::Deck | Target::Lumber(_)
        ) {
            s.extend(self.nums().iter().map(|(k, _)| format!("num{k}")));
        }
        s
    }

    /// The prompt and options for a slot.
    fn slot(&self, slot: &str) -> (String, Vec<(String, String)>) {
        let own = |v: &[(&str, &str)]| -> Vec<(String, String)> {
            v.iter()
                .map(|(l, d)| (l.to_string(), d.to_string()))
                .collect()
        };
        let target = self.settled_target();
        match slot {
            "target" => {
                let mut o: Vec<(String, String)> =
                    self.targets().into_iter().map(|(_, l, d)| (l, d)).collect();
                o.push((
                    SOMETHING_ELSE.into(),
                    "Something else, not in this list or not about the framing".into(),
                ));
                ("What in the design does this change?".into(), o)
            }
            "how" => (
                "Should it go up or down?".into(),
                own(&[
                    (
                        "increase",
                        "Make it bigger, longer, deeper, taller, heavier or steeper",
                    ),
                    (
                        "decrease",
                        "Make it smaller, shorter, shallower, lower, lighter or flatter",
                    ),
                ]),
            ),
            "shape" => (
                "Which roof shape?".into(),
                own(&[
                    ("gable", "Two slopes meeting at a ridge"),
                    ("lean_to", "A single slope, also called a shed roof"),
                ]),
            ),
            "porch" => (
                "What should happen to the porch?".into(),
                own(&[
                    ("add_or_resize", "Add a porch, or keep it and change it"),
                    ("remove", "Take the porch off"),
                ]),
            ),
            "deck" => (
                "What should happen to the porch deck?".into(),
                own(&[
                    ("add", "Put a deck under the porch"),
                    ("remove", "Take the deck out"),
                    ("unmentioned", "The request doesn't mention a deck"),
                ]),
            ),
            "action" => (
                "What should happen to it?".into(),
                own(&[
                    ("resize", "Change its size"),
                    ("move", "Move it along its wall"),
                    ("remove", "Take it out"),
                ]),
            ),
            "kind" => (
                "Window or door?".into(),
                own(&[
                    ("window", "A window"),
                    ("door", "A door"),
                    (
                        "penetration",
                        "A small hole, such as for a mini-split line set or a vent",
                    ),
                ]),
            ),
            "wall" => (
                "Which wall does it go in?".into(),
                self.wall_options()
                    .into_iter()
                    .map(|(_, l, d)| (l, d))
                    .collect(),
            ),
            "count" => (
                "How many?".into(),
                own(&[
                    ("one", "One"),
                    ("two", "Two"),
                    ("three", "Three"),
                    ("four", "Four"),
                ]),
            ),
            "place" => {
                let skip: &[&str] = match target {
                    Some(Target::NewOpening) => &MOVE_ONLY,
                    _ => &NEW_ONLY,
                };
                (
                    "Where along the wall?".into(),
                    own(&PLACES)
                        .into_iter()
                        .filter(|(l, _)| !skip.contains(&l.as_str()))
                        .collect(),
                )
            }
            "ref" => (
                "Which existing door or window does the request put it beside?".into(),
                self.openings()
                    .into_iter()
                    .filter(|((side, i), ..)| target != Some(Target::Opening(*side, *i)))
                    .map(|(_, l, d)| (l, d))
                    .collect(),
            ),
            num => {
                let k: usize = num.trim_start_matches("num").parse().unwrap_or(0);
                let written = self
                    .nums()
                    .get(k)
                    .map(|(_, n)| n.written())
                    .unwrap_or_default();
                let roles: &[(&str, &str)] = match target {
                    Some(Target::Opening(..) | Target::NewOpening) => &[
                        ("width", "How wide the door or window is"),
                        ("height", "How tall the door or window is"),
                        (
                            "distance",
                            "A distance: from a wall end, from another door or window, or how far to move it",
                        ),
                        ("other", "Something else"),
                    ],
                    _ => &[
                        (
                            "new",
                            "What it should become, like the 20' in \"make it 20' wide\"",
                        ),
                        (
                            "amount",
                            "How much to add or take off, like the 2' in \"make it 2' wider\"",
                        ),
                        ("other", "Something else"),
                    ],
                };
                (
                    format!("In this request, what is {written} for?"),
                    own(roles),
                )
            }
        }
    }

    /// The classifier's or user's pick for `slot`, or a question for the
    /// user when there's none, it wasn't confident, or the request doesn't
    /// say.
    fn pick(&self, slot: &str) -> Result<String, Stop> {
        let (prompt, options) = self.slot(slot);
        let answer = self.answers.get(&self.id(slot));
        let unsaid = answer.is_some_and(|a| a.pick == UNSAID && !a.by_user);
        let valid = |a: &&Answer| options.iter().any(|(l, _)| *l == a.pick);
        let reason = match answer.filter(valid) {
            _ if unsaid => "The request doesn't say".to_string(),
            Some(a) if a.by_user => return Ok(a.pick.clone()),
            Some(a) if a.confidence >= CONFIDENT => return Ok(a.pick.clone()),
            Some(a) => format!("Best guess {} at {:.0}%", a.pick, a.confidence * 100.0),
            None => "Not answered yet".to_string(),
        };
        Err(Stop::Ask(Question {
            id: self.id(slot),
            text: self.text.clone(),
            prompt,
            options: options
                .into_iter()
                .map(|(label, description)| Choice { label, description })
                .collect(),
            reason,
        }))
    }

    /// A confident pick for a slot that has a sensible default, without
    /// asking anyone.
    fn maybe(&self, slot: &str) -> Option<String> {
        self.answers
            .get(&self.id(slot))
            .filter(|a| a.pick != UNSAID && (a.by_user || a.confidence >= CONFIDENT))
            .map(|a| a.pick.clone())
    }

    fn target(&self) -> Result<Target, Stop> {
        let pick = self.pick("target")?;
        if pick == SOMETHING_ELSE {
            return Err(Stop::Problem(
                "Sticks can't make that change from a request yet. Try the panel on the left."
                    .into(),
            ));
        }
        Ok(self
            .targets()
            .into_iter()
            .find(|(_, l, _)| *l == pick)
            .map(|(t, ..)| t)
            .expect("picks are checked against the options"))
    }

    fn run(&mut self) -> Out {
        match self.target()? {
            Target::Size(d) => self.size(d),
            Target::Footprint => self.footprint(),
            Target::Pitch => self.pitch_edit(),
            Target::RoofShape => self.roof_shape(),
            Target::Overhangs => self.overhang(),
            Target::Lumber(f) => self.lumber_edit(f),
            Target::Spacing => self.spacing(),
            Target::Porch => self.porch(),
            Target::PorchDepth => self.porch_depth(),
            Target::Deck => self.deck(),
            Target::Opening(side, i) => self.change_opening(side, i),
            Target::NewOpening => self.new_opening(),
        }
    }

    /// The first length written in the clause, read in `unit` when it has
    /// none.
    fn length(&self, unit: Unit) -> Result<Option<Length>, Stop> {
        match self.nums().first() {
            None => Ok(None),
            Some((_, n)) => n.length(unit).map(Some).ok_or_else(|| {
                Stop::Problem(format!("{} doesn't land on a sixteenth.", n.written()))
            }),
        }
    }

    /// The first number given as the new value, and as an amount, read in
    /// `unit` when they have none.
    fn new_and_amount(&self, unit: Unit) -> Result<(Option<Length>, Option<Length>), Stop> {
        let (mut new, mut amount) = (None, None);
        for (k, n) in self.nums() {
            let role = self.pick(&format!("num{k}"))?;
            let slot = match role.as_str() {
                "new" => &mut new,
                "amount" => &mut amount,
                _ => continue,
            };
            if slot.is_none() {
                *slot = Some(n.length(unit).ok_or_else(|| {
                    Stop::Problem(format!("{} doesn't land on a sixteenth.", n.written()))
                })?);
            }
        }
        Ok((new, amount))
    }

    /// The new value of something that is `now`: the number given as the
    /// new value, or bigger or smaller by the amount given, or by one
    /// `step` when there's none.
    fn scalar(&mut self, now: Length, unit: Unit, step: Length) -> Result<Length, Stop> {
        let (new, amount) = self.new_and_amount(unit)?;
        if let Some(l) = new {
            return Ok(l);
        }
        let up = self.pick("how")? == "increase";
        let by = amount.unwrap_or_else(|| {
            self.assumed
                .push(format!("No amount given, so it changes by {step:#}"));
            step
        });
        Ok(if up { now + by } else { now - by })
    }

    fn size(&mut self, dimension: Dimension) -> Out {
        if let Some((w, d)) = self.pair() {
            if dimension != Dimension::Height {
                return self.pair_size(w.clone(), d.clone());
            }
        }
        let b = self.project.building.as_ref().expect("a building target");
        let to = self.scalar(dimension.of(b), Unit::Feet, Length::feet(2))?;
        Ok(vec![Edit::Resize { dimension, to }])
    }

    fn pair_size(&mut self, w: Num, d: Num) -> Out {
        let (Some(w), Some(d)) = (w.length(Unit::Feet), d.length(Unit::Feet)) else {
            return Err(Stop::Problem(
                "Those sizes don't land on a sixteenth.".into(),
            ));
        };
        self.assumed
            .push(format!("{w} by {d} is the width by the depth"));
        Ok(vec![
            Edit::Resize {
                dimension: Dimension::Width,
                to: w,
            },
            Edit::Resize {
                dimension: Dimension::Depth,
                to: d,
            },
        ])
    }

    fn footprint(&mut self) -> Out {
        if let Some((w, d)) = self.pair() {
            return self.pair_size(w.clone(), d.clone());
        }
        let b = self.project.building.as_ref().expect("a building target");
        let (w, d) = (b.width, b.depth);
        let width = self.scalar(w, Unit::Feet, Length::feet(2))?;
        let depth = d + (width - w);
        Ok(vec![
            Edit::Resize {
                dimension: Dimension::Width,
                to: width,
            },
            Edit::Resize {
                dimension: Dimension::Depth,
                to: depth,
            },
        ])
    }

    fn pitch_edit(&mut self) -> Out {
        let roof = self.project.roof.as_ref().expect("a roof target");
        let pitch = match self.pitch() {
            Some(p) => p,
            None => {
                let rise = self.scalar(roof.pitch.rise, Unit::Inches, Length::inches(1))?;
                // The step is 1" of rise per foot, which reads better as a pitch.
                if let Some(a) = self.assumed.last_mut() {
                    if a.starts_with("No amount given") {
                        *a = "No amount given, so it changes by 1 in 12".into();
                    }
                }
                format!("{}:12", format!("{rise:#}").trim_end_matches('"'))
                    .parse()
                    .map_err(Stop::Problem)?
            }
        };
        Ok(vec![Edit::SetPitch { pitch }])
    }

    fn roof_shape(&mut self) -> Out {
        let kind = match self.pick("shape")?.as_str() {
            "gable" => RoofKind::Gable,
            _ => RoofKind::LeanTo,
        };
        let mut edits = vec![Edit::SetRoofKind { kind }];
        if let Some(pitch) = self.pitch() {
            edits.push(Edit::SetPitch { pitch });
        }
        Ok(edits)
    }

    fn overhang(&mut self) -> Out {
        let roof = self.project.roof.as_ref().expect("a roof target");
        let to = self.scalar(roof.front_overhang, Unit::Inches, Length::inches(6))?;
        Ok(vec![Edit::SetOverhang {
            to: to.max(Length::ZERO),
        }])
    }

    fn lumber_edit(&mut self, framing: Framing) -> Out {
        if let Some(size) = self.pair().and_then(|(a, b)| lumber(a, b)) {
            return Ok(vec![Edit::SetLumber { framing, size }]);
        }
        let p = self.project;
        let now = match framing {
            Framing::Studs => p.rules.wall_lumber,
            Framing::Joists => p.floor.as_ref().expect("a floor target").joist,
            Framing::Rafters => p.roof.as_ref().expect("a roof target").rafter,
        };
        let how = self.pick("how")?;
        let i = WIDTHS.iter().position(|w| *w == now.width);
        let next = match (how.as_str(), i) {
            ("increase", Some(i)) => WIDTHS.get(i + 1),
            ("decrease", Some(i)) if i > 0 => WIDTHS.get(i - 1),
            _ => None,
        };
        let Some(&width) = next else {
            return Err(Stop::Problem(format!(
                "Which size? Write it like 2x8. The {} are {now} now.",
                framing.label()
            )));
        };
        let size = Nominal::new(2, width);
        self.assumed.push(format!(
            "One size {}, from {now}",
            if width > now.width { "up" } else { "down" }
        ));
        Ok(vec![Edit::SetLumber { framing, size }])
    }

    fn spacing(&mut self) -> Out {
        let now = self.project.rules.on_center;
        let (new, amount) = self.new_and_amount(Unit::Inches)?;
        let on_center = match (new, amount) {
            (Some(l), _) => l,
            (None, Some(by)) => {
                if self.pick("how")? == "increase" {
                    now + by
                } else {
                    now - by
                }
            }
            (None, None) => {
                let steps = [Length::inches(12), Length::inches(16), Length::inches(24)];
                let i = steps.iter().position(|s| *s == now);
                let next = match (self.pick("how")?.as_str(), i) {
                    ("increase", Some(i)) => steps.get(i + 1),
                    ("decrease", Some(i)) if i > 0 => steps.get(i - 1),
                    _ => None,
                };
                *next
                    .ok_or_else(|| Stop::Problem("Give the spacing, like 24\" on center.".into()))?
            }
        };
        if on_center <= Length::ZERO {
            return Err(Stop::Problem("Spacing has to be more than zero.".into()));
        }
        Ok(vec![Edit::SetSpacing { on_center }])
    }

    fn porch(&mut self) -> Out {
        if self.pick("porch")? == "remove" {
            return match self.project.porch {
                Some(_) => Ok(vec![Edit::RemovePorch]),
                None => Err(Stop::Problem("There's no porch to remove.".into())),
            };
        }
        let mut edits = Vec::new();
        let given = self.length(Unit::Feet)?;
        match (&self.project.porch, given) {
            (_, Some(depth)) => edits.push(Edit::SetPorch { depth }),
            (Some(_), None) => {}
            (None, None) => {
                let d = sticks_core::Porch::default().depth;
                self.assumed
                    .push(format!("No depth given, so the porch is {d} deep"));
                edits.push(Edit::SetPorch { depth: d });
            }
        }
        match self.maybe("deck").as_deref() {
            Some("add") => edits.push(Edit::AddDeck),
            Some("remove")
                if self
                    .project
                    .porch
                    .as_ref()
                    .is_some_and(|p| p.deck.is_some()) =>
            {
                edits.push(Edit::RemoveDeck)
            }
            _ => {}
        }
        if edits.is_empty() {
            return Err(Stop::Problem(
                "Say what to change about the porch, like \"make the porch 8' deep\".".into(),
            ));
        }
        Ok(edits)
    }

    fn porch_depth(&mut self) -> Out {
        let now = self.project.porch.as_ref().expect("a porch target").depth;
        let depth = self.scalar(now, Unit::Feet, Length::feet(1))?;
        Ok(vec![Edit::SetPorch { depth }])
    }

    fn deck(&mut self) -> Out {
        match self.pick("deck")?.as_str() {
            "remove" => Ok(vec![Edit::RemoveDeck]),
            "add" => Ok(vec![Edit::AddDeck]),
            _ => Err(Stop::Problem(
                "Say whether to add the deck or take it out.".into(),
            )),
        }
    }

    fn wall_options(&self) -> Vec<(Side, String, String)> {
        let b = self.project.building.as_ref();
        Side::ALL
            .iter()
            .map(|s| {
                let len = b
                    .map(|b| format!("{} long", s.length(b)))
                    .unwrap_or_default();
                let openings: Vec<&str> = self
                    .project
                    .walls
                    .iter()
                    .filter(|w| w.id == s.id())
                    .flat_map(|w| w.openings.iter().map(|o| kind_name(o.kind)))
                    .collect();
                let has = if openings.is_empty() {
                    "no openings".to_string()
                } else {
                    format!("has {}", openings.join(", "))
                };
                (
                    *s,
                    s.id().to_string(),
                    format!("The {} wall, {len}, {has}", s.id()),
                )
            })
            .collect()
    }

    /// Every opening in a building wall, labeled like `front-2`.
    fn openings(&self) -> Vec<((Side, usize), String, String)> {
        if self.project.building.is_none() {
            return Vec::new();
        }
        Side::ALL
            .iter()
            .flat_map(|s| {
                self.project
                    .walls
                    .iter()
                    .filter(move |w| w.id == s.id())
                    .flat_map(|w| w.openings.iter().enumerate())
                    .map(move |(i, o)| {
                        // Say when it's the only one, so "the door" plainly means it.
                        let kind = kind_name(o.kind);
                        let which = if self.count_of(o.kind) == 1 {
                            format!("The only {kind}, in")
                        } else {
                            format!("The {kind} in")
                        };
                        (
                            (*s, i),
                            format!("{}-{}", s.id(), i + 1),
                            format!(
                                "{which} the {} wall, {} x {}, {} from its left end",
                                s.id(),
                                o.width,
                                o.height,
                                o.offset
                            ),
                        )
                    })
            })
            .collect()
    }

    /// How many openings of `kind` the building's walls have.
    fn count_of(&self, kind: OpeningKind) -> usize {
        self.project
            .walls
            .iter()
            .filter(|w| Side::ALL.iter().any(|s| s.id() == w.id))
            .flat_map(|w| &w.openings)
            .filter(|o| o.kind == kind)
            .count()
    }

    fn opening(&self, side: Side, index: usize) -> sticks_core::Opening {
        self.project
            .walls
            .iter()
            .find(|w| w.id == side.id())
            .and_then(|w| w.openings.get(index))
            .expect("picks are checked against the options")
            .clone()
    }

    /// The opening picked as the one to go beside.
    /// The door or window a request puts something beside. When it's not
    /// clear which, the only door or window there is (in `wall`, if known).
    fn reference(&mut self, wall: Option<Side>) -> Result<(Side, usize), Stop> {
        let ask = match self.pick("ref") {
            Ok(pick) => {
                return Ok(self
                    .openings()
                    .into_iter()
                    .find(|(_, l, _)| *l == pick)
                    .map(|(at, ..)| at)
                    .expect("picks are checked against the options"))
            }
            Err(ask) => ask,
        };
        let (_, options) = self.slot("ref");
        let only: Vec<(Side, usize)> = self
            .openings()
            .into_iter()
            .filter(|(at, label, _)| {
                options.iter().any(|(l, _)| l == label)
                    && wall.is_none_or(|w| w == at.0)
                    && self.opening(at.0, at.1).kind != OpeningKind::Penetration
            })
            .map(|(at, ..)| at)
            .collect();
        let [(side, i)] = only[..] else {
            return Err(ask);
        };
        let there = if wall.is_some() {
            "that wall"
        } else {
            "the design"
        };
        self.assumed.push(format!(
            "Beside the {} in the {} wall, the only door or window in {there}",
            kind_name(self.opening(side, i).kind),
            side.id()
        ));
        Ok((side, i))
    }

    /// The width, height and distance given for an opening. A door's slab
    /// width becomes its rough opening.
    fn opening_numbers(&mut self, kind: OpeningKind) -> Result<OpeningNumbers, Stop> {
        // A bare number up to 8 is feet ("a 3 window"), above that inches.
        let read = |n: &Num| {
            let unit = if !n.has_unit() && n.amount().is_some_and(|a| a > 8.0) {
                Unit::Inches
            } else {
                Unit::Feet
            };
            n.length(unit)
        };
        let mut roles = Vec::new();
        for (k, n) in self.nums() {
            roles.push((self.pick(&format!("num{k}"))?, n.clone()));
        }
        let role = |r: &str| {
            roles
                .iter()
                .find(|(x, _)| x == r)
                .and_then(|(_, n)| read(n))
        };
        let mut width = self.pair().and_then(|(w, _)| read(w)).or(role("width"));
        let height = self.pair().and_then(|(_, h)| read(h)).or(role("height"));
        if let (OpeningKind::Door, Some(w)) = (kind, width) {
            let inches = w.as_inches_f64();
            if inches.fract() == 0.0 && SLAB_WIDTHS.contains(&(inches as i64)) {
                let rough = w + Length::from_sixteenths(40);
                self.assumed
                    .push(format!("A {w} door needs a {rough} rough opening"));
                width = Some(rough);
            }
        }
        Ok((width, height, role("distance")))
    }

    fn new_opening(&mut self) -> Out {
        let b = self.project.building.as_ref().expect("a building target");
        let kind = match self.pick("kind")?.as_str() {
            "door" => OpeningKind::Door,
            "penetration" => OpeningKind::Penetration,
            _ => OpeningKind::Window,
        };
        let count = match self.maybe("count").as_deref() {
            Some("two") => 2,
            Some("three") => 3,
            Some("four") => 4,
            _ => 1,
        };
        let place = self.maybe("place");
        // Beside another opening, the wall is that opening's.
        let beside = match place.as_deref() {
            Some("left_of" | "right_of" | "both_sides") => {
                let wall = self.maybe("wall").and_then(|w| side_of(&w));
                Some(self.reference(wall)?)
            }
            _ => None,
        };
        let wall = match beside {
            Some((side, _)) => side,
            None => side_of(&self.pick("wall")?).expect("picks are checked against the options"),
        };
        let (dw, dh, sill, header) = opening_defaults(kind);
        let (width, height, distance) = self.opening_numbers(kind)?;
        if width.is_none() && height.is_none() {
            self.assumed.push(format!(
                "No size given, so it's the usual {dw} x {dh} rough opening"
            ));
        }
        let new = New {
            wall,
            kind,
            width: width.unwrap_or(dw),
            height: height.unwrap_or(dh),
            sill,
            header,
            replaces: None,
        };
        let wall_len = wall.length(b);
        let room = wall_len - new.width;
        let mut project = self.project.clone();
        let mut edits = Vec::new();

        if let (Some(how), Some((_, index))) = (place.as_deref(), beside) {
            let anchor = self.opening(wall, index);
            let gap = distance.unwrap_or(Length::feet(1));
            let near = kind_name(anchor.kind);
            self.assumed.push(format!(
                "About {gap} from the {near}, or the nearest spot past that which frames"
            ));
            let sides: Vec<&str> = match how {
                "left_of" => vec!["left"; count],
                "right_of" => vec!["right"; count],
                _ => vec!["left", "right"],
            };
            let (mut left_edge, mut right_edge) = (anchor.offset, anchor.offset + anchor.width);
            for side in sides {
                // Start a gap away and move only away, so each opening stays
                // on its side.
                let offsets = if side == "left" {
                    nearest(
                        left_edge - gap - new.width,
                        Length::ZERO,
                        left_edge - new.width,
                    )
                } else {
                    nearest(right_edge + gap, right_edge, room)
                };
                let edit = self.fit(&project, &new, &offsets).map_err(|e| {
                    Stop::Problem(format!("There's no room on the {side} of the {near}: {e}"))
                })?;
                if let Edit::AddOpening { offset, .. } = edit {
                    if side == "left" {
                        left_edge = offset;
                    } else {
                        right_edge = offset + new.width;
                    }
                }
                edit.apply(&mut project)
                    .map_err(|e| Stop::Problem(e.to_string()))?;
                edits.push(edit);
            }
            return Ok(edits);
        }

        // A distance means nothing without the end it's from, so an unsure
        // place is asked rather than dropping the distance and centering.
        let place = match (place, distance) {
            (None, Some(_)) => Some(self.pick("place")?),
            (place, _) => place,
        };
        // Where each opening's left edge should go, and whether it may
        // slide to frame.
        let starts: Vec<(Length, bool)> = match (place.as_deref(), distance) {
            (Some("left_end"), Some(d)) => vec![(d, false)],
            (Some("right_end"), Some(d)) => vec![(wall_len - d - new.width, false)],
            _ if count > 1 => {
                self.assumed
                    .push(format!("{count} spread evenly along the wall"));
                (1..=count as i64)
                    .map(|i| {
                        let center = wall_len.mul_div(i, count as i64 + 1);
                        (center - new.width.mul_div(1, 2), true)
                    })
                    .collect()
            }
            (Some("left_end"), None) => {
                self.assumed
                    .push("Placed 2' from the wall's left end, seen from outside".into());
                vec![(Length::feet(2), true)]
            }
            (Some("right_end"), None) => {
                self.assumed
                    .push("Placed 2' from the wall's right end, seen from outside".into());
                vec![(wall_len - new.width - Length::feet(2), true)]
            }
            _ => {
                self.assumed.push("Centered on the wall".into());
                vec![((wall_len - new.width).mul_div(1, 2), true)]
            }
        };
        for (start, slide) in starts {
            let offsets = if slide {
                nearest(start, Length::ZERO, room)
            } else {
                vec![start]
            };
            let edit = self.fit(&project, &new, &offsets).map_err(Stop::Problem)?;
            edit.apply(&mut project)
                .map_err(|e| Stop::Problem(e.to_string()))?;
            edits.push(edit);
        }
        Ok(edits)
    }

    fn change_opening(&mut self, wall: Side, index: usize) -> Out {
        let b = self
            .project
            .building
            .as_ref()
            .expect("openings are on a building");
        let o = self.opening(wall, index);
        let wall_len = wall.length(b);
        let action = self.pick("action")?;
        if action == "remove" {
            return Ok(vec![Edit::RemoveOpening { wall, index }]);
        }
        let (width, height, distance) = self.opening_numbers(o.kind)?;
        let mut new = New {
            wall,
            kind: o.kind,
            width: o.width,
            height: o.height,
            sill: o.sill_height,
            header: o.header,
            replaces: Some(index),
        };
        let offsets = if action == "resize" {
            if width.is_none() && height.is_none() {
                return Err(Stop::Problem(format!(
                    "Say the new size, like \"make the {} 36x80\".",
                    kind_name(o.kind)
                )));
            }
            new.width = width.unwrap_or(o.width);
            new.height = height.unwrap_or(o.height);
            // Grow or shrink about its middle.
            if new.width != o.width {
                self.assumed.push("Kept centered where it was".into());
            }
            let start = o.offset + o.width.mul_div(1, 2) - new.width.mul_div(1, 2);
            nearest(start, Length::ZERO, wall_len - new.width)
        } else {
            let room = wall_len - o.width;
            let place = self.pick("place")?;
            match (place.as_str(), distance) {
                ("center", _) => nearest((wall_len - o.width).mul_div(1, 2), Length::ZERO, room),
                ("left_end", Some(d)) => vec![d],
                ("right_end", Some(d)) => vec![wall_len - d - o.width],
                ("left_end", None) => {
                    self.assumed
                        .push("Placed 2' from the wall's left end, seen from outside".into());
                    nearest(Length::feet(2), Length::ZERO, room)
                }
                ("right_end", None) => {
                    self.assumed
                        .push("Placed 2' from the wall's right end, seen from outside".into());
                    nearest(room - Length::feet(2), Length::ZERO, room)
                }
                ("shift_left" | "shift_right", d) => {
                    let by = d.unwrap_or_else(|| {
                        self.assumed
                            .push("No distance given, so it moves 1'".into());
                        Length::feet(1)
                    });
                    let at = if place == "shift_left" {
                        o.offset - by
                    } else {
                        o.offset + by
                    };
                    vec![at]
                }
                ("left_of" | "right_of", d) => {
                    let (side, i) = self.reference(Some(wall))?;
                    if side != wall {
                        return Err(Stop::Problem(
                            "Moving an opening to another wall isn't supported yet. Remove it and add a new one there."
                                .into(),
                        ));
                    }
                    let anchor = self.opening(side, i);
                    let gap = d.unwrap_or(Length::feet(1));
                    if place == "left_of" {
                        let edge = anchor.offset;
                        nearest(edge - gap - o.width, Length::ZERO, edge - o.width)
                    } else {
                        let edge = anchor.offset + anchor.width;
                        nearest(edge + gap, edge, room)
                    }
                }
                _ => {
                    return Err(Stop::Problem(
                        "Say where to move it, like \"center it\" or \"2' from the left end\"."
                            .into(),
                    ))
                }
            }
        };
        if offsets
            .iter()
            .all(|&at| at < Length::ZERO || at > wall_len - new.width)
        {
            return Err(Stop::Problem(format!(
                "That would put the {} past the end of the wall.",
                kind_name(o.kind)
            )));
        }
        let project = self.project.clone();
        self.fit(&project, &new, &offsets)
            .map(|e| vec![e])
            .map_err(Stop::Problem)
    }

    /// The first of `offsets` where `new` frames in `project`, trying a door
    /// up to 4' wide with a 2x6 header too. Says what it assumed.
    fn fit(&mut self, project: &Project, new: &New, offsets: &[Length]) -> Result<Edit, String> {
        let header = new.header;
        let headers: &[Nominal] = match new.kind {
            OpeningKind::Door if new.width <= Length::feet(4) => &[header, Nominal::new(2, 6)],
            _ => &[header],
        };
        let first = offsets.first().copied().unwrap_or(Length::ZERO);
        let mut first_problem = None;
        for &h in headers {
            for &at in offsets {
                let edit = new.edit(at, h);
                match wall_problem(project, new.wall, &edit) {
                    None => {
                        if at != first {
                            let (side, by) = if at < first {
                                ("left", first - at)
                            } else {
                                ("right", at - first)
                            };
                            self.assumed
                                .push(format!("Moved {by} {side} to clear the other openings"));
                        }
                        self.assumed.push(if h == header && new.replaces.is_some() {
                            format!("Keeps its {h} header; check it for this span")
                        } else if h == header {
                            format!("{h} header, the app's default; check it for this span")
                        } else {
                            format!(
                                "{h} header so it fits under the top plates; check it for this span"
                            )
                        });
                        return Ok(edit);
                    }
                    Some(p) => {
                        first_problem.get_or_insert(p);
                    }
                }
            }
        }
        Err(format!(
            "A {} x {} {} won't fit in the {} wall: {}",
            new.width,
            new.height,
            kind_name(new.kind),
            new.wall.id(),
            first_problem.unwrap_or_else(|| "there's no room".into())
        ))
    }
}

/// An opening about to be added or changed, before it has a place.
struct New {
    wall: Side,
    kind: OpeningKind,
    width: Length,
    height: Length,
    sill: Length,
    header: Nominal,
    /// The index of the opening it replaces in its wall, when it's an
    /// existing one being resized or moved.
    replaces: Option<usize>,
}

impl New {
    fn edit(&self, offset: Length, header: Nominal) -> Edit {
        if let Some(index) = self.replaces {
            return Edit::ChangeOpening {
                wall: self.wall,
                index,
                kind: self.kind,
                offset,
                width: self.width,
                height: self.height,
                header,
            };
        }
        Edit::AddOpening {
            wall: self.wall,
            kind: self.kind,
            offset,
            width: self.width,
            height: self.height,
            sill_height: self.sill,
            header,
        }
    }
}

/// `start` and then every inch from it within `lo..=hi`, nearest first.
fn nearest(start: Length, lo: Length, hi: Length) -> Vec<Length> {
    let inch = Length::inches(1);
    let mut v = Vec::new();
    if start >= lo && start <= hi {
        v.push(start);
    }
    let mut k = 1;
    while start - inch * k >= lo || start + inch * k <= hi {
        for at in [start - inch * k, start + inch * k] {
            if at >= lo && at <= hi {
                v.push(at);
            }
        }
        k += 1;
    }
    v
}

/// The first reason `edit` keeps its wall from framing in `project`.
fn wall_problem(project: &Project, wall: Side, edit: &Edit) -> Option<String> {
    let mut p = project.clone();
    edit.apply(&mut p).ok()?;
    let prefix = format!("wall {}: ", wall.id());
    crate::framing_problems(&p)
        .into_iter()
        .find_map(|e| e.strip_prefix(&prefix).map(str::to_string))
}
