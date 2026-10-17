//! Turns a request into edits.
//!
//! Rust reads what's notation: it splits the request into clauses and reads
//! the lengths, pitches and sizes in each. Everything that's meaning (what
//! kind of change, which wall, what a number is for) is a fixed-option
//! question from [`questions`], answered by a classifier such as Jev. Rust
//! then turns confident answers into edits; anything the classifier wasn't
//! sure of, or the request doesn't say, comes back as a [`Question`] for the
//! user.

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

/// What a clause is asking for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Intent {
    Resize,
    Pitch,
    RoofKind,
    Overhang,
    Porch,
    Deck,
    AddOpening,
    ChangeOpening,
    RemoveOpening,
    Spacing,
    Lumber,
}

impl Intent {
    const ALL: [Intent; 11] = [
        Intent::Resize,
        Intent::Pitch,
        Intent::RoofKind,
        Intent::Overhang,
        Intent::Porch,
        Intent::Deck,
        Intent::AddOpening,
        Intent::ChangeOpening,
        Intent::RemoveOpening,
        Intent::Spacing,
        Intent::Lumber,
    ];

    fn label(self) -> &'static str {
        match self {
            Intent::Resize => "resize",
            Intent::Pitch => "pitch",
            Intent::RoofKind => "roof_kind",
            Intent::Overhang => "overhang",
            Intent::Porch => "porch",
            Intent::Deck => "deck",
            Intent::AddOpening => "add_opening",
            Intent::ChangeOpening => "change_opening",
            Intent::RemoveOpening => "remove_opening",
            Intent::Spacing => "spacing",
            Intent::Lumber => "lumber",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Intent::Resize => "Change the building's width, depth or height",
            Intent::Pitch => "Make the roof steeper or flatter",
            Intent::RoofKind => "Switch between a lean-to and a gable roof",
            Intent::Overhang => "Change how far the roof runs past the walls",
            Intent::Porch => "Add, resize or remove the front porch",
            Intent::Deck => "Add or remove the deck under the porch",
            Intent::AddOpening => "Add a window or door to a wall",
            Intent::ChangeOpening => "Resize or move an existing window or door",
            Intent::RemoveOpening => "Remove a window, door or other opening",
            Intent::Spacing => "Change how far apart studs, joists and rafters are",
            Intent::Lumber => "Use a different lumber size for studs, joists or rafters",
        }
    }
}

/// The intent option for requests that aren't about framing.
const NOT_FRAMING: &str = "not_framing";

/// What a number in a request is for.
const ROLES: [(&str, &str); 7] = [
    ("size", "The new size of what's being changed"),
    ("change", "How much to add or take away"),
    ("width", "How wide a window or door is"),
    ("height", "How tall a window or door is"),
    (
        "from_left",
        "How far a window or door is from the wall's left end",
    ),
    (
        "from_right",
        "How far a window or door is from the wall's right end",
    ),
    ("other", "Something else"),
];

/// Slots whose answer the request may simply not give ("add a window":
/// which wall?). Their questions offer [`UNSAID`] so the classifier can say
/// so, rather than being sure of a guess.
const MAY_BE_UNSAID: [&str; 7] = [
    "dimension",
    "direction",
    "kind",
    "wall",
    "framing",
    "which",
    "next_to",
];

/// The option for "the request doesn't say".
const UNSAID: &str = "unsaid";

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

/// Every question a classifier should answer about `request`, in one batch.
pub fn questions(project: &Project, request: &str) -> Vec<Ask> {
    let empty = Answers::new();
    clauses(request)
        .into_iter()
        .enumerate()
        .flat_map(|(i, text)| {
            let c = Clause::new(i, text, &empty, project);
            let mut asks = Vec::new();
            for slot in c.slots() {
                let (prompt, mut options) = c.slot(&slot);
                if MAY_BE_UNSAID.contains(&slot.as_str()) {
                    options.push((UNSAID.into(), "The request doesn't say".into()));
                }
                asks.push(Ask::Choice {
                    id: c.id(&slot),
                    text: c.text.clone(),
                    prompt,
                    options: options
                        .into_iter()
                        .map(|(label, description)| Choice { label, description })
                        .collect(),
                });
            }
            asks
        })
        .collect()
}

pub fn interpret(project: &Project, request: &str, answers: &Answers) -> Interpretation {
    let steps = clauses(request)
        .into_iter()
        .enumerate()
        .map(|(i, text)| {
            let mut c = Clause::new(i, text.clone(), answers, project);
            let result = c.run();
            let mut assumed: Vec<String> = Vec::new();
            for a in c.assumed {
                if !assumed.contains(&a) {
                    assumed.push(a);
                }
            }
            let mut step = Step {
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

    /// The slots worth asking about for this clause.
    fn slots(&self) -> Vec<String> {
        let mut s: Vec<String> = [
            "intent",
            "dimension",
            "direction",
            "roof_kind",
            "porch",
            "deck",
            "kind",
            "wall",
            "position",
            "framing",
        ]
        .map(str::to_string)
        .to_vec();
        if !self.openings().is_empty() {
            s.push("which".into());
            s.push("next_to".into());
        }
        s.extend(self.nums().iter().map(|(k, _)| format!("num{k}")));
        s
    }

    /// The prompt and options for a slot.
    fn slot(&self, slot: &str) -> (String, Vec<(String, String)>) {
        let own = |v: &[(&str, &str)]| -> Vec<(String, String)> {
            v.iter()
                .map(|(l, d)| (l.to_string(), d.to_string()))
                .collect()
        };
        let b = self.project.building.as_ref();
        match slot {
            "intent" => {
                let mut o: Vec<(String, String)> = Intent::ALL
                    .iter()
                    .map(|i| (i.label().to_string(), i.description().to_string()))
                    .collect();
                o.push((
                    NOT_FRAMING.into(),
                    "Something else, which Sticks can't change from a request yet".into(),
                ));
                ("What kind of change is this?".into(), o)
            }
            "dimension" => (
                "Which size of the building should change?".into(),
                Dimension::ALL
                    .iter()
                    .map(|d| {
                        let word = match d {
                            Dimension::Width => "along the front wall",
                            Dimension::Depth => "front to back",
                            Dimension::Height => "the front wall's height",
                        };
                        let now = b.map(|b| format!(", now {}", d.of(b))).unwrap_or_default();
                        (
                            d.label().to_string(),
                            format!("The {} ({word}){now}", d.label()),
                        )
                    })
                    .collect(),
            ),
            "direction" => (
                "Should it get bigger or smaller (steeper or flatter, deeper or shallower)?".into(),
                own(&[
                    ("increase", "Bigger, longer, deeper, taller or steeper"),
                    ("decrease", "Smaller, shorter, shallower, lower or flatter"),
                ]),
            ),
            "roof_kind" => (
                "Which roof shape?".into(),
                own(&[
                    ("gable", "Two slopes meeting at a ridge"),
                    ("lean_to", "A single slope, also called a shed roof"),
                ]),
            ),
            "porch" => (
                "What should happen to the porch?".into(),
                own(&[
                    ("add_or_resize", "Add a porch or change its depth"),
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
                "Which wall?".into(),
                self.wall_options()
                    .into_iter()
                    .map(|(_, l, d)| (l, d))
                    .collect(),
            ),
            "position" => (
                "Where along the wall?".into(),
                own(&[
                    ("centered", "Centered, or no place given"),
                    ("left_end", "Near the wall's left end, seen from outside"),
                    ("right_end", "Near the wall's right end, seen from outside"),
                    (
                        "left_of",
                        "Beside an existing door or window, on its left seen from outside",
                    ),
                    (
                        "right_of",
                        "Beside an existing door or window, on its right seen from outside",
                    ),
                    (
                        "both_sides",
                        "One on each side of an existing door or window",
                    ),
                ]),
            ),
            "framing" => (
                "Which framing members?".into(),
                Framing::ALL
                    .iter()
                    .map(|f| (f.label().to_string(), format!("The {}", f.label())))
                    .collect(),
            ),
            "which" => (
                "Which opening?".into(),
                self.openings()
                    .into_iter()
                    .map(|(_, l, d)| (l, d))
                    .collect(),
            ),
            "next_to" => (
                "If the new opening goes beside an existing one, which one?".into(),
                self.openings()
                    .into_iter()
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
                (
                    format!("In this request, what is {written} for?"),
                    own(&ROLES),
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
            .filter(|a| a.by_user || a.confidence >= CONFIDENT)
            .map(|a| a.pick.clone())
    }

    /// The numbers with a given role.
    fn with_role(&self, roles: &[&str]) -> Result<Vec<(String, Num)>, Stop> {
        let mut out = Vec::new();
        for (k, n) in self.nums() {
            let role = self.pick(&format!("num{k}"))?;
            if roles.contains(&role.as_str()) {
                out.push((role, n.clone()));
            }
        }
        Ok(out)
    }

    fn intent(&self) -> Result<Intent, Stop> {
        let pick = self.pick("intent")?;
        if pick == NOT_FRAMING {
            return Err(Stop::Problem(
                "Sticks can't make that change from a request yet. Try the panel on the left."
                    .into(),
            ));
        }
        Ok(Intent::ALL
            .into_iter()
            .find(|i| i.label() == pick)
            .expect("picks are checked against the options"))
    }

    fn run(&mut self) -> Out {
        match self.intent()? {
            Intent::Resize => self.resize(),
            Intent::Pitch => self.pitch_edit(),
            Intent::RoofKind => self.roof_kind(),
            Intent::Overhang => self.overhang(),
            Intent::Porch => self.porch(),
            Intent::Deck => self.deck(),
            Intent::AddOpening => self.add_opening(),
            Intent::ChangeOpening => self.change_opening(),
            Intent::RemoveOpening => self.remove_opening(),
            Intent::Spacing => self.spacing(),
            Intent::Lumber => self.lumber_edit(),
        }
    }

    /// The new value of something that is `now`: a number given as the new
    /// size, a number given as a change, or with no number, one `step` in
    /// the direction asked for.
    fn scalar(&mut self, now: Length, unit: Unit, step: Length) -> Result<Length, Stop> {
        let given = self.with_role(&["size", "change"])?;
        match given.first() {
            Some((role, n)) => {
                let Some(l) = n.length(unit) else {
                    return Err(Stop::Problem(format!(
                        "{} doesn't land on a sixteenth.",
                        n.written()
                    )));
                };
                if role == "size" {
                    return Ok(l);
                }
                let d = if self.pick("direction")? == "increase" {
                    1
                } else {
                    -1
                };
                Ok(now + l * d)
            }
            None => {
                let d = if self.pick("direction")? == "increase" {
                    1
                } else {
                    -1
                };
                self.assumed
                    .push(format!("No amount given, so it moves by {step:#}"));
                Ok(now + step * d)
            }
        }
    }

    fn resize(&mut self) -> Out {
        let Some(b) = self.project.building.as_ref() else {
            return Err(Stop::Problem(
                "This project has no building to resize.".into(),
            ));
        };
        if let Some((w, d)) = self.pair() {
            let (Some(w), Some(d)) = (w.length(Unit::Feet), d.length(Unit::Feet)) else {
                return Err(Stop::Problem(
                    "Those sizes don't land on a sixteenth.".into(),
                ));
            };
            self.assumed
                .push(format!("{w} by {d} is the width by the depth"));
            return Ok(vec![
                Edit::Resize {
                    dimension: Dimension::Width,
                    to: w,
                },
                Edit::Resize {
                    dimension: Dimension::Depth,
                    to: d,
                },
            ]);
        }
        let dimension = match self.pick("dimension")?.as_str() {
            "width" => Dimension::Width,
            "depth" => Dimension::Depth,
            _ => Dimension::Height,
        };
        let to = self.scalar(dimension.of(b), Unit::Feet, Length::feet(2))?;
        Ok(vec![Edit::Resize { dimension, to }])
    }

    fn pitch_edit(&mut self) -> Out {
        let Some(roof) = self.project.roof.as_ref() else {
            return Err(Stop::Problem("This project has no roof.".into()));
        };
        let pitch = match self.pitch() {
            Some(p) => p,
            None => {
                let rise = self.scalar(roof.pitch.rise, Unit::Inches, Length::inches(1))?;
                format!("{}:12", format!("{rise:#}").trim_end_matches('"'))
                    .parse()
                    .map_err(Stop::Problem)?
            }
        };
        Ok(vec![Edit::SetPitch { pitch }])
    }

    fn roof_kind(&mut self) -> Out {
        if self.project.roof.is_none() {
            return Err(Stop::Problem("This project has no roof.".into()));
        }
        let kind = match self.pick("roof_kind")?.as_str() {
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
        let Some(roof) = self.project.roof.as_ref() else {
            return Err(Stop::Problem("This project has no roof.".into()));
        };
        let to = self.scalar(roof.front_overhang, Unit::Inches, Length::inches(6))?;
        Ok(vec![Edit::SetOverhang {
            to: to.max(Length::ZERO),
        }])
    }

    fn porch(&mut self) -> Out {
        if self.pick("porch")? == "remove" {
            return match self.project.porch {
                Some(_) => Ok(vec![Edit::RemovePorch]),
                None => Err(Stop::Problem("There's no porch to remove.".into())),
            };
        }
        let mut edits = match &self.project.porch {
            Some(p) => {
                let now = p.depth;
                let to = self.scalar(now, Unit::Feet, Length::feet(1))?;
                vec![Edit::SetPorch { depth: to }]
            }
            None => {
                let given = self.with_role(&["size"])?;
                let depth = match given.first().and_then(|(_, n)| n.length(Unit::Feet)) {
                    Some(d) => d,
                    None => {
                        let d = sticks_core::Porch::default().depth;
                        self.assumed
                            .push(format!("No depth given, so the porch is {d} deep"));
                        d
                    }
                };
                vec![Edit::SetPorch { depth }]
            }
        };
        if self.maybe("deck").as_deref() == Some("add") {
            edits.push(Edit::AddDeck);
        }
        Ok(edits)
    }

    fn deck(&mut self) -> Out {
        match self.pick("deck")?.as_str() {
            "remove" => Ok(vec![Edit::RemoveDeck]),
            _ if self.project.porch.is_none() => Err(Stop::Problem(
                "The deck goes under the porch, and there's no porch yet. Try \"add a porch with a deck\".".into(),
            )),
            _ => Ok(vec![Edit::AddDeck]),
        }
    }

    fn spacing(&mut self) -> Out {
        let given = self.with_role(&["size", "change"])?;
        let Some(on_center) = given.first().and_then(|(_, n)| n.length(Unit::Inches)) else {
            return Err(Stop::Problem(
                "Give the spacing, like 24\" on center.".into(),
            ));
        };
        if on_center <= Length::ZERO {
            return Err(Stop::Problem("Spacing has to be more than zero.".into()));
        }
        Ok(vec![Edit::SetSpacing { on_center }])
    }

    fn lumber_edit(&mut self) -> Out {
        let Some(size) = self.pair().and_then(|(a, b)| lumber(a, b)) else {
            return Err(Stop::Problem("Which size? Write it like 2x6.".into()));
        };
        let framing = match self.pick("framing")?.as_str() {
            "studs" => Framing::Studs,
            "joists" => Framing::Joists,
            _ => Framing::Rafters,
        };
        Ok(vec![Edit::SetLumber { framing, size }])
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
        Side::ALL
            .iter()
            .flat_map(|s| {
                self.project
                    .walls
                    .iter()
                    .filter(move |w| w.id == s.id())
                    .flat_map(|w| w.openings.iter().enumerate())
                    .map(move |(i, o)| {
                        (
                            (*s, i),
                            format!("{}-{}", s.id(), i + 1),
                            format!(
                                "The {} on the {} wall, {} wide at {}",
                                kind_name(o.kind),
                                s.id(),
                                o.width,
                                o.offset
                            ),
                        )
                    })
            })
            .collect()
    }

    fn add_opening(&mut self) -> Out {
        let Some(b) = self.project.building.as_ref() else {
            return Err(Stop::Problem(
                "This prototype only adds openings to a building's walls.".into(),
            ));
        };
        let kind = match self.pick("kind")?.as_str() {
            "door" => OpeningKind::Door,
            "penetration" => OpeningKind::Penetration,
            _ => OpeningKind::Window,
        };
        // Beside an existing opening, the wall is that opening's.
        let position = self.maybe("position");
        let beside = match position.as_deref() {
            Some(p @ ("left_of" | "right_of" | "both_sides")) => {
                let pick = self.pick("next_to")?;
                let (at, ..) = self
                    .openings()
                    .into_iter()
                    .find(|(_, l, _)| *l == pick)
                    .expect("picks are checked against the options");
                Some((p.to_string(), at))
            }
            _ => None,
        };
        let wall = match &beside {
            Some((_, (side, _))) => *side,
            None => match self.pick("wall")?.as_str() {
                "front" => Side::Front,
                "back" => Side::Back,
                "left" => Side::Left,
                _ => Side::Right,
            },
        };

        let (dw, dh, sill, header) = opening_defaults(kind);
        let (given_width, given_height, from_left, from_right) = self.opening_numbers(kind)?;
        let width = given_width.unwrap_or(dw);
        let height = given_height.unwrap_or(dh);
        if given_width.is_none() && given_height.is_none() {
            self.assumed.push(format!(
                "No size given, so it's the usual {dw} x {dh} rough opening"
            ));
        }
        let new = New {
            wall,
            kind,
            width,
            height,
            sill,
            header,
            replaces: None,
        };
        let wall_len = wall.length(b);
        let room = wall_len - width;

        if let Some((how, (_, index))) = beside {
            return self.add_beside(new, &how, index);
        }

        let placed = match (from_left, from_right) {
            (Some(at), _) => Some(at),
            (None, Some(at)) => Some(wall_len - at - width),
            (None, None) => None,
        };
        let offset = match placed {
            Some(at) => at,
            None => match position.as_deref() {
                Some("left_end") => {
                    self.assumed
                        .push("Placed 2' from the wall's left end, seen from outside".into());
                    Length::feet(2)
                }
                Some("right_end") => {
                    self.assumed
                        .push("Placed 2' from the wall's right end, seen from outside".into());
                    wall_len - width - Length::feet(2)
                }
                _ => {
                    self.assumed.push("Centered on the wall".into());
                    (wall_len - width).mul_div(1, 2)
                }
            },
        };
        // A given spot stays put; otherwise Rust tries the nearest spots
        // either way.
        let offsets = if placed.is_some() {
            vec![offset]
        } else {
            nearest(offset, Length::ZERO, room)
        };
        let project = self.project.clone();
        self.fit(&project, &new, &offsets)
            .map(|e| vec![e])
            .map_err(Stop::Problem)
    }

    /// New openings beside opening `index` on the new opening's wall: on its
    /// left, its right, or both sides, seen from outside.
    fn add_beside(&mut self, new: New, how: &str, index: usize) -> Out {
        let b = self
            .project
            .building
            .as_ref()
            .expect("checked by the caller");
        let room = new.wall.length(b) - new.width;
        let anchor = self
            .project
            .walls
            .iter()
            .find(|w| w.id == new.wall.id())
            .and_then(|w| w.openings.get(index))
            .expect("picks are checked against the options");
        let (a_left, a_right) = (anchor.offset, anchor.offset + anchor.width);
        let gap = Length::feet(1);
        let near = kind_name(anchor.kind);
        let sides: &[&str] = match how {
            "left_of" => &["left"],
            "right_of" => &["right"],
            _ => &["left", "right"],
        };
        self.assumed.push(format!(
            "About {gap} from the {near}, the nearest spot that frames"
        ));
        let mut project = self.project.clone();
        let mut edits = Vec::new();
        for &side in sides {
            // Start a gap away and move only away from it, so the opening
            // stays on its side.
            let offsets = if side == "left" {
                let start = a_left - gap - new.width;
                nearest(start, Length::ZERO, a_left - new.width)
            } else {
                let start = a_right + gap;
                nearest(start, a_right, room)
            };
            let edit = self.fit(&project, &new, &offsets).map_err(|e| {
                Stop::Problem(format!("There's no room on the {side} of the {near}: {e}"))
            })?;
            edit.apply(&mut project)
                .map_err(|e| Stop::Problem(e.to_string()))?;
            edits.push(edit);
        }
        Ok(edits)
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

    /// The width, height and distances from the left and right ends given
    /// for an opening. A door's slab width becomes its rough opening.
    #[allow(clippy::type_complexity)]
    fn opening_numbers(
        &mut self,
        kind: OpeningKind,
    ) -> Result<
        (
            Option<Length>,
            Option<Length>,
            Option<Length>,
            Option<Length>,
        ),
        Stop,
    > {
        // A bare number up to 8 is feet ("a 3 window"), above that inches.
        let read = |n: &Num| {
            let unit = if !n.has_unit() && n.amount().is_some_and(|a| a > 8.0) {
                Unit::Inches
            } else {
                Unit::Feet
            };
            n.length(unit)
        };
        let nums = self.with_role(&["size", "width", "height", "from_left", "from_right"])?;
        let role = |r: &[&str]| {
            nums.iter()
                .find(|(x, _)| r.contains(&x.as_str()))
                .and_then(|(_, n)| read(n))
        };
        let mut width = self
            .pair()
            .and_then(|(w, _)| read(w))
            .or(role(&["width", "size"]));
        let height = self.pair().and_then(|(_, h)| read(h)).or(role(&["height"]));
        if let (OpeningKind::Door, Some(w)) = (kind, width) {
            let inches = w.as_inches_f64();
            if inches.fract() == 0.0 && SLAB_WIDTHS.contains(&(inches as i64)) {
                let rough = w + Length::from_sixteenths(40);
                self.assumed
                    .push(format!("A {w} door needs a {rough} rough opening"));
                width = Some(rough);
            }
        }
        Ok((width, height, role(&["from_left"]), role(&["from_right"])))
    }

    fn change_opening(&mut self) -> Out {
        let openings = self.openings();
        if openings.is_empty() {
            return Err(Stop::Problem("There are no openings to change.".into()));
        }
        let pick = self.pick("which")?;
        let ((wall, index), ..) = openings
            .into_iter()
            .find(|(_, l, _)| *l == pick)
            .expect("picks are checked against the options");
        let b = self
            .project
            .building
            .as_ref()
            .expect("openings are on a building");
        let o = self
            .project
            .walls
            .iter()
            .find(|w| w.id == wall.id())
            .and_then(|w| w.openings.get(index))
            .expect("picks are checked against the options")
            .clone();
        let (width, height, from_left, from_right) = self.opening_numbers(o.kind)?;
        if width.is_none() && height.is_none() && from_left.is_none() && from_right.is_none() {
            return Err(Stop::Problem(format!(
                "Say the new size or place, like \"make the {} 36x80\".",
                kind_name(o.kind)
            )));
        }
        let new = New {
            wall,
            kind: o.kind,
            width: width.unwrap_or(o.width),
            height: height.unwrap_or(o.height),
            sill: o.sill_height,
            header: o.header,
            replaces: Some(index),
        };
        let room = wall.length(b) - new.width;
        let offsets = match (from_left, from_right) {
            (Some(at), _) => vec![at],
            (None, Some(at)) => vec![wall.length(b) - at - new.width],
            (None, None) => {
                // Grow or shrink about its middle.
                let middle = o.offset + o.width.mul_div(1, 2);
                let start = middle - new.width.mul_div(1, 2);
                if new.width != o.width {
                    self.assumed.push("Kept centered where it was".into());
                }
                nearest(start, Length::ZERO, room)
            }
        };
        let project = self.project.clone();
        self.fit(&project, &new, &offsets)
            .map(|e| vec![e])
            .map_err(Stop::Problem)
    }

    fn remove_opening(&mut self) -> Out {
        let openings = self.openings();
        if openings.is_empty() {
            return Err(Stop::Problem("There are no openings to remove.".into()));
        }
        let pick = self.pick("which")?;
        let ((wall, index), ..) = openings
            .into_iter()
            .find(|(_, l, _)| *l == pick)
            .expect("picks are checked against the options");
        Ok(vec![Edit::RemoveOpening { wall, index }])
    }
}

/// An opening about to be added, before it has a place.
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
