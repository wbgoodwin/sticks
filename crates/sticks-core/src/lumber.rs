//! Dimensional lumber sizes and the members a frame is built from.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::units::Length;

/// A nominal lumber size such as 2x4. Actual dimensions follow the US
/// dimensional-lumber convention (2x4 is 1-1/2" x 3-1/2").
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Nominal {
    pub thickness: u8,
    pub width: u8,
}

impl Nominal {
    pub const fn new(thickness: u8, width: u8) -> Self {
        Nominal { thickness, width }
    }

    /// Actual (dressed) thickness.
    pub fn actual_thickness(self) -> Length {
        actual(self.thickness)
    }

    /// Actual (dressed) width.
    pub fn actual_width(self) -> Length {
        actual(self.width)
    }
}

fn actual(nominal: u8) -> Length {
    match nominal {
        1 => Length::from_sixteenths(12), // 3/4"
        n @ 2..=7 => Length::inches(n as i64) - Length::from_sixteenths(8), // minus 1/2"
        n => Length::inches(n as i64) - Length::from_sixteenths(12), // minus 3/4"
    }
}

impl fmt::Display for Nominal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.thickness, self.width)
    }
}

impl FromStr for Nominal {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (t, w) = s
            .trim()
            .split_once(['x', 'X'])
            .ok_or_else(|| format!("{s:?} is not a lumber size like 2x4"))?;
        let parse = |v: &str| {
            v.trim()
                .parse::<u8>()
                .ok()
                .filter(|n| *n > 0)
                .ok_or_else(|| format!("{s:?} is not a lumber size like 2x4"))
        };
        Ok(Nominal::new(parse(t)?, parse(w)?))
    }
}

impl TryFrom<String> for Nominal {
    type Error = String;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<Nominal> for String {
    fn from(n: Nominal) -> String {
        n.to_string()
    }
}

/// What a member does in the frame. Drives labels, BOM grouping and views.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberKind {
    BottomPlate,
    TopPlate,
    CapPlate,
    Stud,
    KingStud,
    JackStud,
    Cripple,
    Header,
    Sill,
    Blocking,
    RimJoist,
    Joist,
    Beam,
    Ledger,
    Rafter,
    Fascia,
    Post,
}

/// How a member's ends are cut.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cut {
    #[default]
    Square,
    /// Angled end, in tenths of a degree from square.
    Angled {
        tenths_of_degree: i32,
    },
    Birdsmouth,
    Plumb,
}

/// One piece of lumber in the generated frame.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    /// Stable label used on elevations and the cut list, e.g. `W1-S07`.
    pub label: String,
    pub kind: MemberKind,
    pub size: Nominal,
    pub length: Length,
    #[serde(default)]
    pub cuts: Vec<Cut>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_sizes() {
        let two_by_four: Nominal = "2x4".parse().unwrap();
        assert_eq!(two_by_four.actual_thickness().to_string(), "1-1/2\"");
        assert_eq!(two_by_four.actual_width().to_string(), "3-1/2\"");
        assert_eq!(Nominal::new(2, 8).actual_width().to_string(), "7-1/4\"");
        assert_eq!(Nominal::new(1, 4).actual_thickness().to_string(), "3/4\"");
    }

    #[test]
    fn rejects_bad_sizes() {
        for bad in ["2by4", "x4", "0x4", "2x"] {
            assert!(bad.parse::<Nominal>().is_err(), "{bad}");
        }
    }
}
