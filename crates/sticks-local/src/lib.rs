//! A classifier on this machine for plain-English design changes.
//!
//! `sticks-intent` turns a request into a fixed batch of multiple-choice
//! questions and reads the picks back as typed edits. Jev answers those
//! questions over the network; [`Answerer`] is the same job done locally, so
//! edits work with no key and no connection. The questions, the answers and
//! the 80% rule are unchanged: an answerer only ever picks one of the listed
//! options and says how sure it is.
//!
//! With the `llama` feature, [`LocalModel`] answers with a small instruct
//! model through llama.cpp: each question goes in as a lettered list and the
//! confidence is the model's odds over just those letters.

mod prompt;

#[cfg(feature = "llama")]
mod llama;

#[cfg(feature = "llama")]
pub use llama::LocalModel;
pub use prompt::{letters, Ending, Prompt};

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sticks_core::Project;
use sticks_intent::{Answer, Answers, Ask, Choice};

/// The model `npm run model:get` downloads and the app looks for.
pub const DEFAULT_MODEL: &str = "qwen2.5-1.5b-instruct-q4_k_m.gguf";

/// Rounds of questions before giving up: the target, then its own. Matches
/// the app's command bar.
pub const ROUNDS: usize = 3;

/// One multiple-choice question, as `sticks edit --questions` prints it and
/// the app passes it around. Reads an [`Ask`]'s JSON.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Query {
    pub id: String,
    /// The clause of the request it's about.
    pub text: String,
    pub prompt: String,
    pub options: Vec<Choice>,
}

impl From<&Ask> for Query {
    fn from(ask: &Ask) -> Self {
        let Ask::Choice {
            id,
            text,
            prompt,
            options,
        } = ask;
        Query {
            id: id.clone(),
            text: text.clone(),
            prompt: prompt.clone(),
            options: options.clone(),
        }
    }
}

/// A pick with the odds of every option, best first.
#[derive(Clone, Debug, PartialEq)]
pub struct Odds(pub Vec<(String, f64)>);

impl Odds {
    /// From each option's score (a logit), in the options' order.
    pub fn from_logits(query: &Query, logits: &[f32]) -> Self {
        let top = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let exp: Vec<f64> = logits.iter().map(|&l| f64::from(l - top).exp()).collect();
        let sum: f64 = exp.iter().sum();
        let mut odds: Vec<(String, f64)> = query
            .options
            .iter()
            .zip(exp)
            .map(|(o, e)| (o.label.clone(), e / sum))
            .collect();
        odds.sort_by(|a, b| b.1.total_cmp(&a.1));
        Odds(odds)
    }

    pub fn answer(&self) -> Answer {
        let (pick, confidence) = self.0[0].clone();
        Answer {
            pick,
            confidence,
            by_user: false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no local model found: run `npm run model:get` in app/, or set STICKS_MODEL to a .gguf file")]
    NoModel,
    #[error("couldn't load the local model: {0}")]
    Load(String),
    #[error("question {id} has {count} options; the local model takes at most {max}")]
    TooManyOptions {
        id: String,
        count: usize,
        max: usize,
    },
    #[error("the request and design are too long for the local model ({tokens} tokens, room for {room})")]
    TooLong { tokens: usize, room: usize },
    #[error("the local model failed: {0}")]
    Model(String),
}

/// Answers multiple-choice questions about a request, the way Jev does.
pub trait Answerer {
    /// Each question's odds, keyed by question id.
    fn odds(
        &mut self,
        request: &str,
        design: &str,
        queries: &[Query],
    ) -> Result<Vec<(String, Odds)>, Error>;

    /// Each question's pick, keyed by question id.
    fn answer(&mut self, request: &str, design: &str, queries: &[Query]) -> Result<Answers, Error> {
        Ok(self
            .odds(request, design, queries)?
            .into_iter()
            .map(|(id, odds)| (id, odds.answer()))
            .collect())
    }
}

/// Asks `answerer` until `sticks-intent` has nothing more for it, adding to
/// `answers`, and returns every question's odds for a caller that wants the
/// runners-up. What's left goes to the user through [`sticks_intent::interpret`].
pub fn settle(
    answerer: &mut dyn Answerer,
    project: &Project,
    request: &str,
    answers: &mut Answers,
) -> Result<Vec<(String, Odds)>, Error> {
    // The same design JSON Jev reads.
    let design = serde_json::to_string_pretty(&sticks_intent::design(project))
        .map_err(|e| Error::Model(e.to_string()))?;
    let mut all = Vec::new();
    for _ in 0..ROUNDS {
        let asks = sticks_intent::questions(project, request, answers);
        if asks.is_empty() {
            break;
        }
        let queries: Vec<Query> = asks.iter().map(Query::from).collect();
        for (id, odds) in answerer.odds(request, &design, &queries)? {
            answers.insert(id.clone(), odds.answer());
            all.push((id, odds));
        }
    }
    Ok(all)
}

/// The model to load: `STICKS_MODEL` if set, else [`DEFAULT_MODEL`] in the
/// first of `dirs` that has it.
pub fn find_model(dirs: &[&Path]) -> Result<PathBuf, Error> {
    if let Some(path) = std::env::var_os("STICKS_MODEL") {
        let path = PathBuf::from(path);
        return if path.is_file() {
            Ok(path)
        } else {
            Err(Error::Load(format!(
                "STICKS_MODEL={} isn't a file",
                path.display()
            )))
        };
    }
    dirs.iter()
        .map(|d| d.join(DEFAULT_MODEL))
        .find(|p| p.is_file())
        .ok_or(Error::NoModel)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn query(labels: &[&str]) -> Query {
        Query {
            id: "0.target".into(),
            text: "make it 20' wide".into(),
            prompt: "What in the design does this change?".into(),
            options: labels
                .iter()
                .map(|l| Choice {
                    label: l.to_string(),
                    description: format!("The {l}"),
                })
                .collect(),
        }
    }

    #[test]
    fn odds_are_a_softmax_over_the_options_best_first() {
        let q = query(&["width", "depth", "height"]);
        let odds = Odds::from_logits(&q, &[1.0, 3.0, 1.0]);
        assert_eq!(odds.0[0].0, "depth");
        let total: f64 = odds.0.iter().map(|o| o.1).sum();
        assert!((total - 1.0).abs() < 1e-9);
        let a = odds.answer();
        assert_eq!(a.pick, "depth");
        assert!(a.confidence > 0.75 && a.confidence < 0.8);
        assert!(!a.by_user);
    }

    #[test]
    fn reads_the_questions_sticks_edit_prints() {
        let project = Project::from_toml(include_str!("../../../examples/shed.toml")).unwrap();
        let asks = sticks_intent::questions(&project, "add a window", &Answers::new());
        let json = serde_json::to_string(&asks).unwrap();
        let read: Vec<Query> = serde_json::from_str(&json).unwrap();
        let direct: Vec<Query> = asks.iter().map(Query::from).collect();
        assert_eq!(read, direct);
        assert!(!read.is_empty());
    }

    /// Always picks the option whose label matches a word in the clause,
    /// else the first.
    struct Echo;

    impl Answerer for Echo {
        fn odds(
            &mut self,
            _: &str,
            _: &str,
            queries: &[Query],
        ) -> Result<Vec<(String, Odds)>, Error> {
            Ok(queries
                .iter()
                .map(|q| {
                    let logits: Vec<f32> = q
                        .options
                        .iter()
                        .map(|o| {
                            if q.text.contains(o.label.as_str()) {
                                10.0
                            } else {
                                0.0
                            }
                        })
                        .collect();
                    (q.id.clone(), Odds::from_logits(q, &logits))
                })
                .collect())
        }
    }

    #[test]
    fn settles_the_target_then_its_own_questions() {
        const REQUEST: &str = "change the width to 20'";
        let project = Project::from_toml(include_str!("../../../examples/shed.toml")).unwrap();
        let mut answers = Answers::new();
        let odds = settle(&mut Echo, &project, REQUEST, &mut answers).unwrap();
        assert_eq!(answers["0.target"].pick, "width");
        // A second round asked the width's own questions.
        assert!(odds.len() > 1, "{odds:?}");
        assert!(sticks_intent::questions(&project, REQUEST, &answers).is_empty());
    }

    #[test]
    fn a_missing_model_says_how_to_get_one() {
        if std::env::var_os("STICKS_MODEL").is_some() {
            return;
        }
        let err = find_model(&[Path::new("/nonexistent")]).unwrap_err();
        assert!(err.to_string().contains("model:get"));
    }
}
