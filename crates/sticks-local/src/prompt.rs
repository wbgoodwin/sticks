//! The text a local model reads, in the ChatML format Qwen and many other
//! small instruct models use.
//!
//! Every question about one request shares the same opening (the design and
//! the request), so the model reads it once and only the question itself
//! changes. The reply starts right after the prompt, and its first token is
//! the letter of an option.
//!
//! Small models lean toward options by where they sit in the list, so each
//! question is also asked with its options in reverse, and the two readings
//! are averaged.

use crate::{Error, Query};

const SYSTEM: &str = "You read requests to change the design of a small wood-framed \
building, such as a shed, and answer one multiple-choice question about each. \
Pick the option the request means. If the request doesn't say, pick the option \
that says so. Reply with only the option's letter.";

/// Option letters, in order: A to Z, then a to z.
pub fn letters() -> impl Iterator<Item = char> {
    ('A'..='Z').chain('a'..='z')
}

/// The prompt for one request: an opening shared by all its questions, then
/// for each question, its endings: one per order of the options.
#[derive(Clone, Debug, PartialEq)]
pub struct Prompt {
    pub opening: String,
    pub endings: Vec<Vec<Ending>>,
}

/// One question with its options in one order.
#[derive(Clone, Debug, PartialEq)]
pub struct Ending {
    pub text: String,
    /// The index of the option under each letter.
    pub order: Vec<usize>,
}

impl Prompt {
    pub fn new(request: &str, design: &str, queries: &[Query]) -> Result<Self, Error> {
        let opening = format!(
            "<|im_start|>system\n{SYSTEM}<|im_end|>\n<|im_start|>user\n\
             The design now:\n{}\n\nThe request: \"{request}\"\n\n",
            design.trim_end()
        );
        let endings = queries.iter().map(ending).collect::<Result<_, _>>()?;
        Ok(Prompt { opening, endings })
    }
}

fn ending(q: &Query) -> Result<Vec<Ending>, Error> {
    let max = letters().count();
    if q.options.len() > max {
        return Err(Error::TooManyOptions {
            id: q.id.clone(),
            count: q.options.len(),
            max,
        });
    }
    let forward: Vec<usize> = (0..q.options.len()).collect();
    let mut orders = vec![forward.clone()];
    if q.options.len() > 1 {
        orders.push(forward.into_iter().rev().collect());
    }
    Ok(orders
        .into_iter()
        .map(|order| {
            let mut text = format!("About \"{}\": {}\n", q.text, q.prompt);
            for (letter, &i) in letters().zip(&order) {
                // The label too, since the design names openings by it.
                let o = &q.options[i];
                text += &format!("{letter}. {}: {}\n", o.label, o.description);
            }
            text += "Answer with one letter.<|im_end|>\n<|im_start|>assistant\n";
            Ending { text, order }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::query;

    #[test]
    fn letters_each_option_after_the_shared_opening() {
        let q = query(&["width", "depth"]);
        let p = Prompt::new("make it 20' wide", "Shed\n  building: 18' wide\n", &[q]).unwrap();
        assert!(p.opening.ends_with("The request: \"make it 20' wide\"\n\n"));
        assert!(p.opening.contains("building: 18' wide\n\nThe request"));
        let [forward, reversed] = &p.endings[0][..] else {
            panic!("two orders")
        };
        let e = &forward.text;
        assert!(e.starts_with("About \"make it 20' wide\": What in the design does this change?\n"));
        assert!(e.contains("A. width: The width\nB. depth: The depth\n"));
        assert!(e.ends_with("<|im_start|>assistant\n"));
        assert!(reversed
            .text
            .contains("A. depth: The depth\nB. width: The width\n"));
        assert_eq!(reversed.order, [1, 0]);
    }

    #[test]
    fn too_many_options_is_an_error_not_a_wrong_letter() {
        let labels: Vec<String> = (0..60).map(|i| format!("o{i}")).collect();
        let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let err = Prompt::new("x", "", &[query(&refs)]).unwrap_err();
        assert!(matches!(
            err,
            Error::TooManyOptions {
                count: 60,
                max: 52,
                ..
            }
        ));
    }
}
