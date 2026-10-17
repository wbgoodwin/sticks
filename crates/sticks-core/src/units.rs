//! Lengths stored as integer sixteenths of an inch, so repeated additions never
//! drift the way floating-point inches would.

use std::fmt;
use std::iter::Sum;
use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Sixteenths of an inch per inch.
pub const PER_INCH: i64 = 16;
/// Sixteenths of an inch per foot.
pub const PER_FOOT: i64 = PER_INCH * 12;

/// A length in sixteenths of an inch.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Length(i64);

impl Length {
    pub const ZERO: Length = Length(0);

    pub const fn from_sixteenths(n: i64) -> Self {
        Length(n)
    }

    pub const fn inches(n: i64) -> Self {
        Length(n * PER_INCH)
    }

    pub const fn feet(n: i64) -> Self {
        Length(n * PER_FOOT)
    }

    pub const fn sixteenths(self) -> i64 {
        self.0
    }

    /// Length in decimal inches, for display and geometry only.
    pub fn as_inches_f64(self) -> f64 {
        self.0 as f64 / PER_INCH as f64
    }

    pub const fn abs(self) -> Self {
        Length(self.0.abs())
    }

    /// How you'd read it off a tape: inches under 10' (`92-5/8"`), feet and
    /// inches beyond (`18'`, `10' 3-1/2"`).
    pub fn to_tape_string(self) -> String {
        if self.abs() < Length::feet(10) {
            format!("{self:#}")
        } else {
            self.to_string()
        }
    }
}

impl Add for Length {
    type Output = Length;
    fn add(self, rhs: Length) -> Length {
        Length(self.0 + rhs.0)
    }
}

impl AddAssign for Length {
    fn add_assign(&mut self, rhs: Length) {
        self.0 += rhs.0;
    }
}

impl Sub for Length {
    type Output = Length;
    fn sub(self, rhs: Length) -> Length {
        Length(self.0 - rhs.0)
    }
}

impl SubAssign for Length {
    fn sub_assign(&mut self, rhs: Length) {
        self.0 -= rhs.0;
    }
}

impl Neg for Length {
    type Output = Length;
    fn neg(self) -> Length {
        Length(-self.0)
    }
}

impl Mul<i64> for Length {
    type Output = Length;
    fn mul(self, rhs: i64) -> Length {
        Length(self.0 * rhs)
    }
}

impl Sum for Length {
    fn sum<I: Iterator<Item = Length>>(iter: I) -> Length {
        iter.fold(Length::ZERO, Add::add)
    }
}

/// Formats as feet and inches with a reduced fraction, e.g. `10' 3-1/2"`.
/// Use the alternate form (`{:#}`) for inches only, e.g. `92-5/8"`.
impl fmt::Display for Length {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.0 < 0 { "-" } else { "" };
        let total = self.0.unsigned_abs() as i64;
        let (feet, rest) = if f.alternate() {
            (0, total)
        } else {
            (total / PER_FOOT, total % PER_FOOT)
        };
        let inches = rest / PER_INCH;
        let mut num = rest % PER_INCH;
        let mut den = PER_INCH;
        while num != 0 && num % 2 == 0 {
            num /= 2;
            den /= 2;
        }
        let frac = (num != 0).then(|| format!("{num}/{den}"));

        write!(f, "{sign}")?;
        if feet != 0 {
            write!(f, "{feet}'")?;
            if inches == 0 && frac.is_none() {
                return Ok(());
            }
            write!(f, " ")?;
        }
        match (inches, frac) {
            (0, Some(fr)) => write!(f, "{fr}\""),
            (i, Some(fr)) => write!(f, "{i}-{fr}\""),
            (i, None) => write!(f, "{i}\""),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("can't read {input:?} as a length: {reason}")]
pub struct ParseLengthError {
    pub input: String,
    pub reason: &'static str,
}

/// Parses builder notation: `10'`, `10' 3-1/2"`, `10'3.5"`, `92 5/8`, `5/8"`,
/// `3.25`. A bare number is inches. Values must land on a sixteenth.
impl FromStr for Length {
    type Err = ParseLengthError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let err = |reason| ParseLengthError {
            input: input.to_string(),
            reason,
        };
        let s = input.trim();
        let (negative, s) = match s.strip_prefix('-') {
            Some(rest) => (true, rest.trim_start()),
            None => (false, s),
        };
        if s.is_empty() {
            return Err(err("it is empty"));
        }

        let (feet_part, inch_part) = match s.find('\'') {
            Some(i) => (Some(&s[..i]), &s[i + 1..]),
            None => (None, s),
        };

        let mut total = 0i64;
        if let Some(ft) = feet_part {
            if ft.trim().is_empty() {
                return Err(err("no number before the foot mark"));
            }
            total += parse_amount(ft.trim(), PER_FOOT).ok_or_else(|| err("bad feet value"))?;
        }

        let inch_part = inch_part.trim().trim_start_matches('-').trim();
        let inch_part = inch_part
            .strip_suffix('"')
            .or_else(|| inch_part.strip_suffix("in"))
            .unwrap_or(inch_part)
            .trim();
        if !inch_part.is_empty() {
            total += parse_amount(inch_part, PER_INCH).ok_or_else(|| err("bad inch value"))?;
        } else if feet_part.is_none() {
            return Err(err("no number found"));
        }

        Ok(Length(if negative { -total } else { total }))
    }
}

/// Parses `3`, `3.5`, `3-1/2`, `3 1/2` or `1/2` in units of `scale`
/// sixteenths. Returns `None` unless the result is a whole sixteenth.
fn parse_amount(s: &str, scale: i64) -> Option<i64> {
    if s.is_empty() {
        return Some(0);
    }
    let (whole, frac) = match s.find('/') {
        None => (s, None),
        Some(_) => match s.rfind(['-', ' ']) {
            Some(i) => (s[..i].trim(), Some(s[i + 1..].trim())),
            None => ("", Some(s)),
        },
    };

    let mut total = 0i64;
    if !whole.is_empty() {
        total += parse_decimal(whole, scale)?;
    }
    if let Some(frac) = frac {
        let (n, d) = frac.split_once('/')?;
        let n: i64 = n.trim().parse().ok()?;
        let d: i64 = d.trim().parse().ok()?;
        if d <= 0 || n < 0 || (n * scale) % d != 0 {
            return None;
        }
        total += n * scale / d;
    }
    Some(total)
}

fn parse_decimal(s: &str, scale: i64) -> Option<i64> {
    if !s.chars().all(|c| c.is_ascii_digit() || c == '.') || s == "." {
        return None;
    }
    let (int, dec) = s.split_once('.').unwrap_or((s, ""));
    let int: i64 = if int.is_empty() { 0 } else { int.parse().ok()? };
    let mut total = int * scale;
    if !dec.is_empty() {
        let den = 10i64.checked_pow(dec.len() as u32)?;
        let num: i64 = dec.parse().ok()?;
        if (num * scale) % den != 0 {
            return None;
        }
        total += num * scale / den;
    }
    Some(total)
}

/// Project files store lengths as strings in builder notation; plain numbers
/// are read as inches.
impl Serialize for Length {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Length {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Text(String),
            Int(i64),
            Float(f64),
        }
        match Raw::deserialize(d)? {
            Raw::Text(s) => s.parse().map_err(serde::de::Error::custom),
            Raw::Int(i) => Ok(Length::inches(i)),
            Raw::Float(x) => {
                let n = x * PER_INCH as f64;
                if (n - n.round()).abs() > 1e-9 {
                    return Err(serde::de::Error::custom(format!(
                        "{x} inches is not a whole sixteenth"
                    )));
                }
                Ok(Length(n.round() as i64))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn p(s: &str) -> Length {
        s.parse().unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn parses_builder_notation() {
        assert_eq!(
            p("10' 3-1/2\""),
            Length::from_sixteenths(10 * 192 + 3 * 16 + 8)
        );
        assert_eq!(p("10'3-1/2\""), p("10' 3 1/2"));
        assert_eq!(p("10'"), Length::feet(10));
        assert_eq!(p("92-5/8\""), Length::from_sixteenths(92 * 16 + 10));
        assert_eq!(p("92 5/8"), p("92-5/8\""));
        assert_eq!(p("5/8\""), Length::from_sixteenths(10));
        assert_eq!(p("3.5"), Length::from_sixteenths(56));
        assert_eq!(p("1.5in"), Length::from_sixteenths(24));
        assert_eq!(p("8' 0\""), Length::feet(8));
        assert_eq!(p("1/2'"), Length::inches(6));
        assert_eq!(p("-1/8"), Length::from_sixteenths(-2));
    }

    #[test]
    fn rejects_bad_input() {
        for bad in ["", "abc", "1/32", "3.01", "1/0", "'", "10' x", "1.2.3"] {
            assert!(bad.parse::<Length>().is_err(), "{bad:?} should not parse");
        }
    }

    #[test]
    fn displays_reduced_fractions() {
        assert_eq!(p("10' 3-1/2\"").to_string(), "10' 3-1/2\"");
        assert_eq!(Length::feet(8).to_string(), "8'");
        assert_eq!(p("92-5/8").to_string(), "7' 8-5/8\"");
        assert_eq!(format!("{:#}", p("92-5/8")), "92-5/8\"");
        assert_eq!(p("1/8").to_string(), "1/8\"");
        assert_eq!(Length::ZERO.to_string(), "0\"");
        assert_eq!((-p("1-1/4")).to_string(), "-1-1/4\"");
    }

    #[test]
    fn tape_strings() {
        assert_eq!(p("92-5/8").to_tape_string(), "92-5/8\"");
        assert_eq!(p("115-1/2").to_tape_string(), "115-1/2\"");
        assert_eq!(p("18'").to_tape_string(), "18'");
    }

    #[test]
    fn sums_without_drift() {
        let kerf = p("1/8");
        let total: Length = std::iter::repeat_n(kerf, 1000).sum();
        assert_eq!(total, Length::inches(125));
    }

    proptest! {
        #[test]
        fn display_round_trips(n in -100_000i64..100_000) {
            let len = Length::from_sixteenths(n);
            prop_assert_eq!(len.to_string().parse::<Length>().unwrap(), len);
            prop_assert_eq!(format!("{len:#}").parse::<Length>().unwrap(), len);
        }
    }
}
