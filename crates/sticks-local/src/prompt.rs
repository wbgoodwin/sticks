//! The text a local model reads, in the ChatML format Qwen and many other
//! small instruct models use.
//!
//! Every question about one request shares the same opening (the design and
//! the request), so the model reads it once and only the question itself
//! changes. The reply starts right after the prompt, and its first token is
//! the letter of an option.

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
/// one ending per question.
#[derive(Clone, Debug, PartialEq)]
pub struct Prompt {
    pub opening: String,
    pub endings: Vec<String>,
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

fn ending(q: &Query) -> Result<String, Error> {
    let max = letters().count();
    if q.options.len() > max {
        return Err(Error::TooManyOptions {
            id: q.id.clone(),
            count: q.options.len(),
            max,
        });
    }
    let mut out = format!("About \"{}\": {}\n", q.text, q.prompt);
    for (letter, o) in letters().zip(&q.options) {
        out += &format!("{letter}. {}\n", o.description);
    }
    out += "Answer with one letter.<|im_end|>\n<|im_start|>assistant\n";
    Ok(out)
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
        let e = &p.endings[0];
        assert!(e.starts_with("About \"make it 20' wide\": What in the design does this change?\n"));
        assert!(e.contains("A. The width\nB. The depth\n"));
        assert!(e.ends_with("<|im_start|>assistant\n"));
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
