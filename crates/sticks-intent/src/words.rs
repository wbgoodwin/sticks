//! Splits a request into clauses and each clause into words and the values a
//! builder writes: lengths (`20'`, `3 ft 6 in`, `38-1/2"`), pitches (`4:12`,
//! `4 in 12`) and pairs (`12x16`, `20 by 16`, `2x6`).

use sticks_core::{Length, Nominal, Pitch};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Feet,
    Inches,
}

/// A number as written, with its unit if one was given.
#[derive(Clone, Debug, PartialEq)]
pub struct Num {
    /// Builder notation when `unit` is `None` and the text had a foot or
    /// inch mark; otherwise just the amount, like `20` or `3-1/2`.
    text: String,
    unit: Option<Unit>,
    marked: bool,
}

impl Num {
    /// Whether the request said what unit this is in.
    pub fn has_unit(&self) -> bool {
        self.marked || self.unit.is_some()
    }

    /// The length, reading a bare number in `default` units.
    pub fn length(&self, default: Unit) -> Option<Length> {
        if self.marked {
            return self.text.parse().ok();
        }
        let mark = match self.unit.unwrap_or(default) {
            Unit::Feet => "'",
            Unit::Inches => "\"",
        };
        format!("{}{mark}", self.text).parse().ok()
    }

    /// The amount as a plain number, ignoring units.
    pub fn amount(&self) -> Option<f64> {
        let t = self.text.trim_end_matches(['"', '\'']);
        match t.split_once('/') {
            None => t.parse().ok(),
            Some(_) => self.length(Unit::Inches).map(|l| l.as_inches_f64()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Word(String),
    Num(Num),
    Pitch(Pitch),
    /// `AxB` or `A by B`.
    Pair(Num, Num),
}

impl Tok {
    pub fn word(&self) -> Option<&str> {
        match self {
            Tok::Word(w) => Some(w),
            _ => None,
        }
    }
}

/// Splits on commas, semicolons, sentence ends and joining words.
pub fn clauses(request: &str) -> Vec<String> {
    let text = normalize(request);
    let mut out = vec![text];
    for sep in [
        ";",
        ". ",
        ", and ",
        ",",
        " and then ",
        " then ",
        " and ",
        " also ",
        " plus ",
    ] {
        out = out
            .iter()
            .flat_map(|c| c.split(sep).map(str::to_string).collect::<Vec<_>>())
            .collect();
    }
    out.into_iter()
        .map(|c| {
            c.trim()
                .trim_end_matches(['.', '!', '?'])
                .trim()
                .to_string()
        })
        .filter(|c| !c.is_empty())
        .collect()
}

fn normalize(s: &str) -> String {
    s.to_lowercase()
        .replace(['\u{2019}', '\u{2018}', '\u{2032}'], "'")
        .replace(['\u{201c}', '\u{201d}', '\u{2033}'], "\"")
        .replace("lean to", "lean-to")
        .replace("on-center", "on center")
        .replace("o.c.", "oc")
}

fn is_amount(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | '/' | '-'))
        && s.chars().next().is_some_and(|c| c.is_ascii_digit())
}

fn unit_word(s: &str) -> Option<Unit> {
    match s {
        "'" | "ft" | "foot" | "feet" => Some(Unit::Feet),
        "\"" | "in" | "inch" | "inches" => Some(Unit::Inches),
        _ => None,
    }
}

/// A single written number: `20`, `20'`, `3'6"`, `38-1/2"`.
fn num(s: &str) -> Option<Num> {
    if is_amount(s) {
        return Some(Num {
            text: s.to_string(),
            unit: None,
            marked: false,
        });
    }
    for (suffix, unit) in [("ft", Unit::Feet), ("in", Unit::Inches)] {
        if let Some(a) = s.strip_suffix(suffix).filter(|a| is_amount(a)) {
            return Some(Num {
                text: a.to_string(),
                unit: Some(unit),
                marked: false,
            });
        }
    }
    if s.contains(['\'', '"']) && s.parse::<Length>().is_ok() {
        return Some(Num {
            text: s.to_string(),
            unit: None,
            marked: true,
        });
    }
    None
}

fn pitch(s: &str) -> Option<Pitch> {
    let (rise, run) = s.split_once([':', '/'])?;
    (run == "12" && is_amount(rise))
        .then(|| format!("{rise}:12").parse().ok())
        .flatten()
}

pub fn tokens(clause: &str) -> Vec<Tok> {
    let raw: Vec<String> = clause
        .split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| matches!(c, ',' | '(' | ')' | '!' | '?'))
                .to_string()
        })
        .filter(|w| !w.is_empty())
        .collect();

    let mut out: Vec<Tok> = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        let w = raw[i].as_str();
        let next = raw.get(i + 1).map(String::as_str);
        let after = raw.get(i + 2).map(String::as_str);

        // `4:12`, `4/12`, `4 in 12`, `4 on 12`
        if let Some(p) = pitch(w) {
            out.push(Tok::Pitch(p));
            i += 1;
            continue;
        }
        if is_amount(w) && matches!(next, Some("in" | "on" | "/" | ":")) && after == Some("12") {
            if let Some(p) = pitch(&format!("{w}:12")) {
                out.push(Tok::Pitch(p));
                i += 3;
                continue;
            }
        }
        // `12x16`, `2x6`, `36"x48"`
        if let Some((a, b)) = w.split_once('x') {
            if let (Some(a), Some(b)) = (num(a), num(b)) {
                out.push(Tok::Pair(a, b));
                i += 1;
                continue;
            }
        }
        if let Some(mut n) = num(w) {
            i += 1;
            // `3 1/2` → `3-1/2`
            if let Some(f) = raw
                .get(i)
                .filter(|f| !n.has_unit() && f.contains('/') && is_amount(f))
            {
                n.text = format!("{}-{f}", n.text);
                i += 1;
            }
            if let Some(u) = raw
                .get(i)
                .and_then(|u| unit_word(u))
                .filter(|_| !n.has_unit())
            {
                n.unit = Some(u);
                i += 1;
            }
            // `3 ft 6 in`, `20' 6"`
            if n.length(Unit::Feet).is_some()
                && (n.unit == Some(Unit::Feet) || n.text.ends_with('\''))
            {
                if let Some(inch) = raw.get(i).and_then(|t| num(t)) {
                    let unit_next = raw.get(i + 1).and_then(|u| unit_word(u));
                    let is_inches = inch.text.ends_with('"')
                        || inch.unit == Some(Unit::Inches)
                        || unit_next == Some(Unit::Inches);
                    if is_inches {
                        let feet = n.length(Unit::Feet).unwrap_or(Length::ZERO);
                        if let Some(inches) = inch.length(Unit::Inches) {
                            n = Num {
                                text: format!("{}", feet + inches),
                                unit: None,
                                marked: true,
                            };
                            i += if unit_next.is_some() { 2 } else { 1 };
                        }
                    }
                }
            }
            // `20 by 16`, `20 x 16`
            if matches!(raw.get(i).map(String::as_str), Some("by" | "x")) {
                if let Some(b) = raw.get(i + 1).and_then(|t| num(t)) {
                    let mut b = b;
                    i += 2;
                    if let Some(u) = raw
                        .get(i)
                        .and_then(|u| unit_word(u))
                        .filter(|_| !b.has_unit())
                    {
                        b.unit = Some(u);
                        i += 1;
                    }
                    out.push(Tok::Pair(n, b));
                    continue;
                }
            }
            out.push(Tok::Num(n));
            continue;
        }
        out.push(Tok::Word(w.to_string()));
        i += 1;
    }
    out
}

/// A pair that reads as lumber, like `2x6` or `5/4x6`, rather than a size.
pub fn lumber(a: &Num, b: &Num) -> Option<Nominal> {
    if a.has_unit() || b.has_unit() {
        return None;
    }
    let thick_ok = matches!(a.text.as_str(), "1" | "2" | "3" | "4" | "6" | "5/4" | "6/4");
    let wide_ok = b.text.parse::<u8>().is_ok_and(|w| (2..=12).contains(&w));
    (thick_ok && wide_ok)
        .then(|| format!("{}x{}", a.text, b.text).parse().ok())
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn len(t: &Tok) -> Length {
        match t {
            Tok::Num(n) => n.length(Unit::Feet).unwrap(),
            other => panic!("not a number: {other:?}"),
        }
    }

    #[test]
    fn splits_clauses() {
        assert_eq!(
            clauses("Make it 20' wide and add a window, then a 4:12 roof."),
            ["make it 20' wide", "add a window", "a 4:12 roof"]
        );
    }

    #[test]
    fn reads_lengths_in_many_spellings() {
        let t = tokens("20 feet 3 ft 6 in 38-1/2\" 10' 3\" 3 1/2 in");
        let lens: Vec<String> = t.iter().map(|t| len(t).to_string()).collect();
        assert_eq!(lens, ["20'", "3' 6\"", "3' 2-1/2\"", "10' 3\"", "3-1/2\""]);
    }

    #[test]
    fn reads_pitches_and_pairs() {
        let t = tokens("a 4 in 12 roof 6/12 12x16 20 by 16 2x6");
        assert!(matches!(t[1], Tok::Pitch(p) if p.to_string() == "4:12"));
        assert!(matches!(t[3], Tok::Pitch(p) if p.to_string() == "6:12"));
        assert!(matches!(&t[4], Tok::Pair(a, _) if a.length(Unit::Feet) == Some(Length::feet(12))));
        assert!(matches!(&t[5], Tok::Pair(_, b) if b.length(Unit::Feet) == Some(Length::feet(16))));
        let Tok::Pair(a, b) = &t[6] else { panic!() };
        assert_eq!(lumber(a, b).unwrap().to_string(), "2x6");
        let Tok::Pair(a, b) = &t[4] else { panic!() };
        assert_eq!(lumber(a, b), None);
    }
}
