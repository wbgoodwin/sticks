//! Turns a request into edits. Everything that words and numbers settle is
//! settled here; what's left is a [`Question`] with a fixed set of options,
//! for a classifier or the user to answer.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sticks_core::{Length, Nominal, OpeningKind, Pitch, Project, RoofKind};

use crate::edit::{kind_name, Dimension, Edit, Framing, Side};
use crate::words::{clauses, lumber, tokens, Num, Tok, Unit};

/// Answers to earlier questions, by question id.
pub type Answers = BTreeMap<String, String>;

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

/// A choice between fixed options.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Question {
    /// Stable across runs of the same request: `{clause}.{slot}`.
    pub id: String,
    /// The clause it's about.
    pub text: String,
    pub prompt: String,
    pub options: Vec<Choice>,
    /// The clause has no words that bear on this, so a classifier could
    /// only guess. Ask the user.
    pub for_user: bool,
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
    RemoveOpening,
    Spacing,
    Lumber,
}

impl Intent {
    const ALL: [Intent; 10] = [
        Intent::Resize,
        Intent::Pitch,
        Intent::RoofKind,
        Intent::Overhang,
        Intent::Porch,
        Intent::Deck,
        Intent::AddOpening,
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
            Intent::RemoveOpening => "Remove a window, door or other opening",
            Intent::Spacing => "Change how far apart studs, joists and rafters are",
            Intent::Lumber => "Use a different lumber size for studs, joists or rafters",
        }
    }
}

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

/// The intent option for requests that aren't about framing.
const NOT_FRAMING: &str = "not_framing";

pub fn interpret(project: &Project, request: &str, answers: &Answers) -> Interpretation {
    let steps = clauses(request)
        .into_iter()
        .enumerate()
        .map(|(i, text)| {
            let mut c = Clause {
                index: i,
                text: text.clone(),
                toks: tokens(&text),
                answers,
                project,
                assumed: Vec::new(),
            };
            let result = c.run();
            let mut step = Step {
                text,
                edits: Vec::new(),
                assumed: c.assumed,
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

const REMOVE: &[&str] = &[
    "remove", "delete", "drop", "no", "without", "lose", "rid", "ditch", "skip",
];
const GROW: &[&str] = &[
    "wider", "deeper", "taller", "higher", "longer", "bigger", "larger", "more", "extend",
    "increase", "add", "steeper", "raise",
];
const SHRINK: &[&str] = &[
    "narrower",
    "shallower",
    "shorter",
    "lower",
    "smaller",
    "less",
    "reduce",
    "decrease",
    "shrink",
    "flatter",
    "trim",
];

/// Words that carry no answer on their own.
const FILLER: &[&str] = &[
    "a", "an", "the", "it", "its", "this", "that", "to", "in", "into", "on", "at", "of", "for",
    "with", "by", "from", "me", "i", "we", "my", "our", "some", "new", "another", "one", "put",
    "make", "set", "want", "would", "like", "please", "let", "can", "could", "should", "be",
    "need", "is", "change", "just", "bit", "little", "lot", "shed",
];
/// Words the interpreter already reads for meaning.
const READ: &[&str] = &[
    "window",
    "door",
    "penetration",
    "opening",
    "wall",
    "side",
    "porch",
    "deck",
    "roof",
    "pitch",
    "slope",
    "overhang",
    "stud",
    "joist",
    "rafter",
    "spacing",
];

impl Clause<'_> {
    fn has(&self, words: &[&str]) -> bool {
        self.toks
            .iter()
            .filter_map(Tok::word)
            .any(|w| words.contains(&w) || words.contains(&w.trim_end_matches('s')))
    }

    fn has_phrase(&self, phrase: &str) -> bool {
        let words: Vec<&str> = self.toks.iter().map(|t| t.word().unwrap_or("#")).collect();
        words.join(" ").contains(phrase)
    }

    fn nums(&self) -> Vec<&Num> {
        self.toks
            .iter()
            .filter_map(|t| match t {
                Tok::Num(n) => Some(n),
                _ => None,
            })
            .collect()
    }

    fn pair(&self) -> Option<(&Num, &Num)> {
        self.toks.iter().find_map(|t| match t {
            Tok::Pair(a, b) => Some((a, b)),
            _ => None,
        })
    }

    fn lumber(&self) -> Option<Nominal> {
        self.pair().and_then(|(a, b)| lumber(a, b))
    }

    fn pitch(&self) -> Option<Pitch> {
        self.toks.iter().find_map(|t| match t {
            Tok::Pitch(p) => Some(*p),
            _ => None,
        })
    }

    /// +1 to grow, -1 to shrink, 0 for neither.
    fn direction(&self) -> i64 {
        match (self.has(GROW), self.has(SHRINK)) {
            (true, false) => 1,
            (false, true) => -1,
            _ => 0,
        }
    }

    fn answer(&self, slot: &str) -> Option<&str> {
        self.answers
            .get(&format!("{}.{slot}", self.index))
            .map(String::as_str)
    }

    fn ask(&self, slot: &str, prompt: &str, options: Vec<Choice>) -> Stop {
        Stop::Ask(Question {
            id: format!("{}.{slot}", self.index),
            text: self.text.clone(),
            prompt: prompt.to_string(),
            options,
            for_user: !self.has_unread_words(),
        })
    }

    /// Whether any word is left once filler and the words already read are
    /// set aside. "add a window" has none, so which wall is a pure guess.
    fn has_unread_words(&self) -> bool {
        self.toks.iter().filter_map(Tok::word).any(|w| {
            let w = w.trim_end_matches('s');
            ![FILLER, GROW, SHRINK, REMOVE, READ]
                .iter()
                .any(|list| list.contains(&w))
        })
    }

    /// The answer to `slot` among `labels`, or a question offering them.
    fn choose<T: Copy>(
        &self,
        slot: &str,
        prompt: &str,
        options: &[(T, String, String)],
    ) -> Result<T, Stop> {
        if let Some(a) = self.answer(slot) {
            if let Some((v, ..)) = options.iter().find(|(_, l, _)| l == a) {
                return Ok(*v);
            }
        }
        let choices = options
            .iter()
            .map(|(_, label, description)| Choice {
                label: label.clone(),
                description: description.clone(),
            })
            .collect();
        Err(self.ask(slot, prompt, choices))
    }

    fn intent(&self) -> Result<Intent, Stop> {
        if let Some(i) = self
            .answer("intent")
            .and_then(|a| Intent::ALL.into_iter().find(|i| i.label() == a))
        {
            return Ok(i);
        }
        let opening = self.has(&["window", "door", "penetration", "opening"]);
        let found = if opening && self.has(REMOVE) {
            Some(Intent::RemoveOpening)
        } else if opening {
            Some(Intent::AddOpening)
        } else if self.has(&["porch"]) {
            Some(Intent::Porch)
        } else if self.has(&["deck"]) {
            Some(Intent::Deck)
        } else if self.has(&["overhang", "eave"]) {
            Some(Intent::Overhang)
        } else if self.has(&["gable", "lean-to", "leanto"])
            || self.has_phrase("shed roof")
            || self.has_phrase("single slope")
        {
            Some(Intent::RoofKind)
        } else if self.pitch().is_some() || self.has(&["pitch", "slope", "steeper", "flatter"]) {
            Some(Intent::Pitch)
        } else if self.has_phrase("on center") || self.has(&["oc", "spacing", "spaced"]) {
            Some(Intent::Spacing)
        } else if self.lumber().is_some() || self.has(&["stud", "joist", "rafter"]) {
            Some(Intent::Lumber)
        } else if self.dimension_words().next().is_some()
            || self.pair().is_some()
            || self.has(&["bigger", "smaller", "larger", "resize"])
        {
            Some(Intent::Resize)
        } else {
            None
        };
        match found {
            Some(i) => Ok(i),
            None => {
                if self.answer("intent") == Some(NOT_FRAMING) {
                    return Err(Stop::Problem(
                        "That isn't a change Sticks can make to the framing.".into(),
                    ));
                }
                let mut options: Vec<_> = Intent::ALL
                    .iter()
                    .map(|i| (Some(*i), i.label().to_string(), i.description().to_string()))
                    .collect();
                options.push((
                    None,
                    NOT_FRAMING.into(),
                    "Something else, not a change to the framing".into(),
                ));
                match self.choose("intent", "What kind of change is this?", &options)? {
                    Some(i) => Ok(i),
                    None => Err(Stop::Problem(
                        "That isn't a change Sticks can make to the framing.".into(),
                    )),
                }
            }
        }
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
            Intent::RemoveOpening => self.remove_opening(),
            Intent::Spacing => self.spacing(),
            Intent::Lumber => self.lumber_edit(),
        }
    }

    fn dimension_words(&self) -> impl Iterator<Item = Dimension> + '_ {
        self.toks
            .iter()
            .filter_map(Tok::word)
            .filter_map(|w| match w {
                "wide" | "width" | "wider" | "narrower" | "narrow" => Some(Dimension::Width),
                "deep" | "depth" | "deeper" | "shallower" => Some(Dimension::Depth),
                "tall" | "height" | "taller" | "higher" | "high" | "shorter" | "raise"
                | "lower" => Some(Dimension::Height),
                _ => None,
            })
    }

    /// `to` relative to `now`: a length after a grow or shrink word is a
    /// change, a bare grow or shrink word moves by `step`.
    fn amount(
        &self,
        now: Length,
        given: Option<Length>,
        step: Length,
        what: &str,
        [grow, shrink]: [&str; 2],
    ) -> Result<Length, Stop> {
        // "raise it to 10'" names the new size, even after a grow word.
        let to_named = self
            .toks
            .windows(2)
            .any(|w| w[0].word() == Some("to") && matches!(w[1], Tok::Num(_)));
        let dir = if to_named { 0 } else { self.direction() };
        match (given, dir) {
            (Some(l), 0) => Ok(l),
            (Some(l), d) => Ok(now + l * d),
            // "Shed rain better" says which way, not how far.
            (None, 0) => {
                let options = [(1, grow), (-1, shrink)].map(|(d, word)| {
                    (
                        d,
                        word.to_string(),
                        format!("Make the {what} {word}, by {step:#}"),
                    )
                });
                let d = self.choose("direction", &format!("{grow} or {shrink}?"), &options)?;
                Ok(now + step * d)
            }
            (None, d) => Ok(now + step * d),
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
        let dims: Vec<Dimension> = self.dimension_words().collect();
        let nums: Vec<Length> = self
            .nums()
            .iter()
            .filter_map(|n| n.length(Unit::Feet))
            .collect();
        if dims.len() > 1 && dims.len() == nums.len() && self.direction() == 0 {
            return Ok(dims
                .into_iter()
                .zip(nums)
                .map(|(dimension, to)| Edit::Resize { dimension, to })
                .collect());
        }
        let dimension = match dims.first() {
            Some(d) => *d,
            None => {
                let options: Vec<_> = Dimension::ALL
                    .iter()
                    .map(|d| {
                        let word = match d {
                            Dimension::Width => "along the front wall",
                            Dimension::Depth => "front to back",
                            Dimension::Height => "the front wall's height",
                        };
                        (
                            *d,
                            d.label().to_string(),
                            format!("The {} ({word}), now {}", d.label(), d.of(b)),
                        )
                    })
                    .collect();
                self.choose("dimension", "Which size should change?", &options)?
            }
        };
        let step = Length::feet(2);
        let to = self.amount(
            dimension.of(b),
            nums.first().copied(),
            step,
            dimension.label(),
            ["bigger", "smaller"],
        )?;
        if nums.is_empty() {
            self.assumed
                .push(format!("No amount given, so it moves by {step}"));
        }
        Ok(vec![Edit::Resize { dimension, to }])
    }

    fn pitch_edit(&mut self) -> Out {
        let Some(roof) = self.project.roof.as_ref() else {
            return Err(Stop::Problem("This project has no roof.".into()));
        };
        let pitch = match self.pitch() {
            Some(p) => p,
            None => {
                let given = self.nums().first().and_then(|n| n.length(Unit::Inches));
                let step = Length::inches(1);
                let rise = self.amount(
                    roof.pitch.rise,
                    given,
                    step,
                    "pitch",
                    ["steeper", "flatter"],
                )?;
                if given.is_none() {
                    self.assumed.push(format!(
                        "No amount given, so the rise moves by {step} per foot"
                    ));
                }
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
        let kind = if self.has(&["gable"]) {
            RoofKind::Gable
        } else {
            RoofKind::LeanTo
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
        let given = self.nums().first().and_then(|n| n.length(Unit::Inches));
        let step = Length::inches(6);
        let to = self.amount(
            roof.front_overhang,
            given,
            step,
            "overhang",
            ["longer", "shorter"],
        )?;
        if given.is_none() {
            self.assumed
                .push(format!("No amount given, so it moves by {step}"));
        }
        Ok(vec![Edit::SetOverhang {
            to: to.max(Length::ZERO),
        }])
    }

    fn porch(&mut self) -> Out {
        if self.has(REMOVE) {
            return match self.project.porch {
                Some(_) if self.has(&["deck"]) => Ok(vec![Edit::RemoveDeck]),
                Some(_) => Ok(vec![Edit::RemovePorch]),
                None => Err(Stop::Problem("There's no porch to remove.".into())),
            };
        }
        let given = self.nums().first().and_then(|n| n.length(Unit::Feet));
        let mut edits = match (&self.project.porch, given) {
            (Some(p), given) if self.direction() != 0 || given.is_some() => {
                let step = Length::feet(1);
                let to = self.amount(p.depth, given, step, "porch", ["deeper", "shallower"])?;
                if given.is_none() {
                    self.assumed
                        .push(format!("No amount given, so it moves by {step}"));
                }
                vec![Edit::SetPorch { depth: to }]
            }
            (Some(_), _) if self.has(&["deck"]) => vec![],
            (Some(_), _) => {
                return Err(Stop::Problem(
                    "There's already a porch. Say how deep it should be.".into(),
                ))
            }
            (None, Some(depth)) => vec![Edit::SetPorch { depth }],
            (None, None) => {
                let depth = sticks_core::Porch::default().depth;
                self.assumed
                    .push(format!("No depth given, so the porch is {depth} deep"));
                vec![Edit::SetPorch { depth }]
            }
        };
        if self.has(&["deck"]) {
            edits.push(Edit::AddDeck);
        }
        Ok(edits)
    }

    fn deck(&mut self) -> Out {
        if self.has(REMOVE) {
            return Ok(vec![Edit::RemoveDeck]);
        }
        if self.project.porch.is_none() {
            return Err(Stop::Problem("The deck goes under the porch, and there's no porch yet. Try \"add a porch with a deck\".".into()));
        }
        Ok(vec![Edit::AddDeck])
    }

    fn spacing(&mut self) -> Out {
        let Some(on_center) = self.nums().first().and_then(|n| n.length(Unit::Inches)) else {
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
        let Some(size) = self.lumber() else {
            return Err(Stop::Problem("Which size? Write it like 2x6.".into()));
        };
        let framing = if self.has(&["stud", "wall"]) {
            Framing::Studs
        } else if self.has(&["joist", "floor"]) {
            Framing::Joists
        } else if self.has(&["rafter", "roof"]) {
            Framing::Rafters
        } else {
            let options: Vec<_> = Framing::ALL
                .iter()
                .map(|f| {
                    (
                        *f,
                        f.label().to_string(),
                        format!("Use {size} for the {}", f.label()),
                    )
                })
                .collect();
            self.choose("framing", "Which members should change size?", &options)?
        };
        Ok(vec![Edit::SetLumber { framing, size }])
    }

    /// Side words in order, with whether the next word is "wall".
    fn sides(&self) -> Vec<(Side, bool)> {
        let side = |w: &str| match w {
            "front" => Some(Side::Front),
            "back" | "rear" => Some(Side::Back),
            "left" => Some(Side::Left),
            "right" => Some(Side::Right),
            _ => None,
        };
        self.toks
            .iter()
            .enumerate()
            .filter_map(|(i, t)| {
                let s = side(t.word()?)?;
                let wall = self
                    .toks
                    .get(i + 1)
                    .and_then(Tok::word)
                    .is_some_and(|w| w.starts_with("wall"));
                Some((s, wall))
            })
            .collect()
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

    fn add_opening(&mut self) -> Out {
        let Some(b) = self.project.building.as_ref() else {
            return Err(Stop::Problem(
                "This prototype only adds openings to a building's walls.".into(),
            ));
        };
        let kind = if self.has(&["door"]) {
            OpeningKind::Door
        } else if self.has(&["window"]) {
            OpeningKind::Window
        } else if self.has(&["penetration"]) {
            OpeningKind::Penetration
        } else {
            let options = [OpeningKind::Window, OpeningKind::Door]
                .map(|k| (k, kind_name(k).to_string(), format!("A {}", kind_name(k))));
            self.choose("kind", "Window or door?", &options)?
        };

        let sides = self.sides();
        let wall = match sides.iter().find(|(_, w)| *w).or(if sides.len() == 1 {
            sides.first()
        } else {
            None
        }) {
            Some((s, _)) => *s,
            None => self.choose("wall", "Which wall should it go in?", &self.wall_options())?,
        };

        // A length right before "from" places the opening; the rest size it.
        let words: Vec<Option<&str>> = self.toks.iter().map(Tok::word).collect();
        let from = words.iter().position(|w| *w == Some("from"));
        let placed = from.and_then(|f| match self.toks.get(f.checked_sub(1)?) {
            Some(Tok::Num(n)) => n
                .length(Unit::Inches)
                .filter(|_| n.has_unit())
                .or_else(|| n.length(Unit::Feet)),
            _ => None,
        });
        let from_right = from.is_some_and(|f| words[f..].contains(&Some("right")));
        let sizes: Vec<&Num> = self
            .toks
            .iter()
            .enumerate()
            .filter(|(i, _)| from.is_none_or(|f| *i + 1 != f))
            .filter_map(|(_, t)| match t {
                Tok::Num(n) => Some(n),
                _ => None,
            })
            .collect();
        // A bare number up to 8 is feet ("a 3 window"), above that inches.
        let read = |n: &Num| {
            let unit = if !n.has_unit() && n.amount().is_some_and(|a| a > 8.0) {
                Unit::Inches
            } else {
                Unit::Feet
            };
            n.length(unit)
        };

        let (dw, dh, sill, header) = opening_defaults(kind);
        let (mut width, mut height) = match (self.pair(), sizes.first()) {
            (Some((w, h)), _) => (read(w).unwrap_or(dw), read(h).unwrap_or(dh)),
            (None, Some(w)) => (read(w).unwrap_or(dw), dh),
            (None, None) => {
                self.assumed.push(format!(
                    "No size given, so it's the usual {dw} x {dh} rough opening"
                ));
                (dw, dh)
            }
        };
        if kind == OpeningKind::Door && (self.pair().is_some() || !sizes.is_empty()) {
            let w = width.as_inches_f64();
            if w.fract() == 0.0 && SLAB_WIDTHS.contains(&(w as i64)) {
                let slab = width;
                width = slab + Length::from_sixteenths(40);
                if self.pair().is_none() {
                    height = dh;
                }
                self.assumed
                    .push(format!("A {slab} door needs a {width} rough opening"));
            }
        }

        let wall_len = wall.length(b);
        let slide = placed.is_none();
        let offset = match placed {
            Some(at) if from_right => wall_len - at - width,
            Some(at) => at,
            None => {
                let near = sides
                    .iter()
                    .find(|(s, w)| !*w && *s != wall && matches!(s, Side::Left | Side::Right));
                match near {
                    Some((Side::Left, _)) => {
                        self.assumed
                            .push("Placed 2' from the wall's left end, seen from outside".into());
                        Length::feet(2)
                    }
                    Some(_) => {
                        self.assumed
                            .push("Placed 2' from the wall's right end, seen from outside".into());
                        wall_len - width - Length::feet(2)
                    }
                    None => {
                        self.assumed.push("Centered on the wall".into());
                        (wall_len - width).mul_div(1, 2)
                    }
                }
            }
        };
        let edit = |offset, header| Edit::AddOpening {
            wall,
            kind,
            offset,
            width,
            height,
            sill_height: sill,
            header,
        };

        // Rust tries the nearest spots and, for a door up to 4' wide, one
        // header size down, and keeps the first that frames.
        let headers: &[Nominal] = match kind {
            OpeningKind::Door if width <= Length::feet(4) => &[header, Nominal::new(2, 6)],
            _ => &[header],
        };
        let room = wall_len - width;
        let offsets: Vec<Length> = if slide {
            let inch = Length::inches(1);
            let mut v = vec![offset];
            let mut k = 1;
            while offset - inch * k >= Length::ZERO || offset + inch * k <= room {
                for at in [offset - inch * k, offset + inch * k] {
                    if at >= Length::ZERO && at <= room {
                        v.push(at);
                    }
                }
                k += 1;
            }
            v
        } else {
            vec![offset]
        };
        let first_problem = self.wall_problem(wall, &edit(offset, header));
        let Some(first_problem) = first_problem else {
            self.assumed.push(format!(
                "{header} header, the app's default; check it for this span"
            ));
            return Ok(vec![edit(offset, header)]);
        };
        for &h in headers {
            for &at in &offsets {
                if self.wall_problem(wall, &edit(at, h)).is_none() {
                    if at != offset {
                        let side = if at < offset { "left" } else { "right" };
                        let by = if at < offset {
                            offset - at
                        } else {
                            at - offset
                        };
                        self.assumed
                            .push(format!("Moved {by} {side} to clear the other openings"));
                    }
                    self.assumed.push(if h == header {
                        format!("{h} header, the app's default; check it for this span")
                    } else {
                        format!(
                            "{h} header so it fits under the top plates; check it for this span"
                        )
                    });
                    return Ok(vec![edit(at, h)]);
                }
            }
        }
        Err(Stop::Problem(format!(
            "A {width} x {height} {} won't fit in the {} wall: {first_problem}",
            kind_name(kind),
            wall.id()
        )))
    }

    /// The first reason `edit` keeps its wall from framing.
    fn wall_problem(&self, wall: Side, edit: &Edit) -> Option<String> {
        let mut p = self.project.clone();
        edit.apply(&mut p).ok()?;
        let prefix = format!("wall {}: ", wall.id());
        crate::framing_problems(&p)
            .into_iter()
            .find_map(|e| e.strip_prefix(&prefix).map(str::to_string))
    }

    fn remove_opening(&mut self) -> Out {
        let kind = if self.has(&["door"]) {
            Some(OpeningKind::Door)
        } else if self.has(&["window"]) {
            Some(OpeningKind::Window)
        } else if self.has(&["penetration"]) {
            Some(OpeningKind::Penetration)
        } else {
            None
        };
        let sides = self.sides();
        let wall = sides
            .iter()
            .find(|(_, w)| *w)
            .or(if sides.len() == 1 {
                sides.first()
            } else {
                None
            })
            .map(|(s, _)| *s);
        let found: Vec<(Side, usize, String)> = Side::ALL
            .iter()
            .filter(|s| wall.is_none_or(|w| w == **s))
            .flat_map(|s| {
                self.project
                    .walls
                    .iter()
                    .filter(move |w| w.id == s.id())
                    .flat_map(|w| w.openings.iter().enumerate())
                    .filter(|(_, o)| kind.is_none_or(|k| k == o.kind))
                    .map(move |(i, o)| {
                        (
                            *s,
                            i,
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
            .collect();
        let what = kind.map(kind_name).unwrap_or("opening");
        let place = wall
            .map(|w| format!(" on the {} wall", w.id()))
            .unwrap_or_default();
        match found.len() {
            0 => Err(Stop::Problem(format!(
                "There's no {what}{place} to remove."
            ))),
            1 => Ok(vec![Edit::RemoveOpening {
                wall: found[0].0,
                index: found[0].1,
            }]),
            _ if self.has(&["all", "every", "both"]) => {
                // Highest index first, so earlier removals don't shift later ones.
                let mut all: Vec<_> = found
                    .iter()
                    .map(|(s, i, _)| Edit::RemoveOpening {
                        wall: *s,
                        index: *i,
                    })
                    .collect();
                all.reverse();
                Ok(all)
            }
            _ => {
                let options: Vec<_> = found
                    .iter()
                    .map(|(s, i, d)| ((*s, *i), format!("{}-{}", s.id(), i + 1), d.clone()))
                    .collect();
                let (wall, index) =
                    self.choose("which", &format!("Which {what}{place}?"), &options)?;
                Ok(vec![Edit::RemoveOpening { wall, index }])
            }
        }
    }
}
