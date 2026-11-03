//! Span checks: how far a framing member may reach between supports under
//! the code's span tables, and the checks that hold a framed design to them.
//!
//! Rafters are checked against the International Residential Code's rafter
//! tables (2018 and 2021 IRC, Tables R802.4.1(1) to (3), which the 2021
//! Virginia Residential Code adopts unchanged), No. 2 grade:
//!
//! - Table R802.4.1(1): roof live load 20 psf, no ceiling on the rafters
//!   (deflection L/180).
//! - Table R802.4.1(2): roof live load 20 psf, ceiling attached (L/240).
//! - Table R802.4.1(3): ground snow load 30 psf, no ceiling (L/180).
//!
//! Ground snow of 20 psf or less uses the live load tables; up to 30 psf
//! uses the 30 psf snow table. Heavier snow, and a ceiling under a roof that
//! carries snow (Table R802.4.1(4)), are reported as not covered rather than
//! guessed. Spans are the clear level distance between the faces of the
//! supports. Spacings and dead loads between columns take the column that's
//! never less safe. Spans the tables mark as over 26' are taken as 26'.
//!
//! Sources: the 2018 Seattle Residential Code's chapter 8, which prints the
//! IRC tables unamended, cross-checked against the No. 2 rows quoted by
//! reuven.tools' rafter span calculator. Transcribed by hand; verify against
//! your copy of the code before a permit hangs on one.
//!
//! Each check names the members it covers, so the same mechanism can carry
//! joist and header checks later.

use sticks_core::{
    FramingSpecies, Length, Nominal, Project, Roof, DEFAULT_GROUND_SNOW_PSF, DEFAULT_ROOF_DEAD_PSF,
};

use crate::code::{check, CodeCheck};
use crate::layout::Placed;
use crate::roof::RafterDetail;

const fn fi(feet: i64, inches: i64) -> Length {
    Length::from_sixteenths((feet * 12 + inches) * 16)
}

/// A rafter table: by species (southern pine, spruce-pine-fir, Douglas
/// fir-larch, hem-fir), then spacing (12", 16", 19.2", 24"), then dead load
/// (10 psf, 20 psf), then size (2x4, 2x6, 2x8, 2x10, 2x12).
type RafterTable = [[[[Length; 5]; 2]; 4]; 4];

/// The rafter tables' spacing columns, as their widest spacing in
/// sixteenths: 12", 16", 19.2" (19-3/16") and 24".
const SPACINGS_16: [i64; 4] = [192, 256, 307, 384];
const SPACING_NAMES: [&str; 4] = ["12\"", "16\"", "19.2\"", "24\""];

/// Table R802.4.1(1), No. 2 grade.
const LIVE_20_NO_CEILING: RafterTable = [
    // Southern pine: 12", 16", 19.2", 24" on center.
    [
        [
            [fi(10, 4), fi(15, 7), fi(19, 8), fi(23, 5), fi(26, 0)],
            [fi(9, 0), fi(13, 6), fi(17, 1), fi(20, 3), fi(23, 10)],
        ],
        [
            [fi(9, 0), fi(13, 6), fi(17, 1), fi(20, 3), fi(23, 10)],
            [fi(7, 9), fi(11, 8), fi(14, 9), fi(17, 6), fi(20, 8)],
        ],
        [
            [fi(8, 2), fi(12, 3), fi(15, 7), fi(18, 6), fi(21, 9)],
            [fi(7, 1), fi(10, 8), fi(13, 6), fi(16, 0), fi(18, 10)],
        ],
        [
            [fi(7, 4), fi(11, 0), fi(13, 11), fi(16, 6), fi(19, 6)],
            [fi(6, 4), fi(9, 6), fi(12, 1), fi(14, 4), fi(16, 10)],
        ],
    ],
    // Spruce-pine-fir: 12", 16", 19.2", 24" on center.
    [
        [
            [fi(10, 4), fi(16, 3), fi(21, 0), fi(25, 8), fi(26, 0)],
            [fi(9, 10), fi(14, 4), fi(18, 2), fi(22, 3), fi(25, 9)],
        ],
        [
            [fi(9, 5), fi(14, 4), fi(18, 2), fi(22, 3), fi(25, 9)],
            [fi(8, 6), fi(12, 5), fi(15, 9), fi(19, 3), fi(22, 4)],
        ],
        [
            [fi(8, 10), fi(13, 1), fi(16, 7), fi(20, 3), fi(23, 6)],
            [fi(7, 9), fi(11, 4), fi(14, 4), fi(17, 7), fi(20, 4)],
        ],
        [
            [fi(7, 6), fi(11, 9), fi(14, 10), fi(18, 2), fi(21, 0)],
            [fi(6, 11), fi(10, 2), fi(12, 10), fi(15, 8), fi(18, 3)],
        ],
    ],
    // Douglas fir-larch: 12", 16", 19.2", 24" on center.
    [
        [
            [fi(10, 10), fi(16, 10), fi(21, 4), fi(26, 0), fi(26, 0)],
            [fi(10, 0), fi(14, 7), fi(18, 5), fi(22, 6), fi(26, 0)],
        ],
        [
            [fi(9, 10), fi(14, 7), fi(18, 5), fi(22, 6), fi(26, 0)],
            [fi(8, 7), fi(12, 7), fi(16, 0), fi(19, 6), fi(22, 7)],
        ],
        [
            [fi(9, 1), fi(13, 3), fi(16, 10), fi(20, 7), fi(23, 10)],
            [fi(7, 10), fi(11, 6), fi(14, 7), fi(17, 10), fi(20, 8)],
        ],
        [
            [fi(8, 2), fi(11, 11), fi(15, 1), fi(18, 5), fi(21, 4)],
            [fi(7, 0), fi(10, 4), fi(13, 0), fi(15, 11), fi(18, 6)],
        ],
    ],
    // Hem-fir: 12", 16", 19.2", 24" on center.
    [
        [
            [fi(10, 1), fi(15, 11), fi(20, 8), fi(25, 3), fi(26, 0)],
            [fi(9, 8), fi(14, 2), fi(17, 11), fi(21, 11), fi(25, 5)],
        ],
        [
            [fi(9, 2), fi(14, 2), fi(17, 11), fi(21, 11), fi(25, 5)],
            [fi(8, 5), fi(12, 3), fi(15, 6), fi(18, 11), fi(22, 0)],
        ],
        [
            [fi(8, 8), fi(12, 11), fi(16, 4), fi(20, 0), fi(23, 2)],
            [fi(7, 8), fi(11, 2), fi(14, 2), fi(17, 4), fi(20, 1)],
        ],
        [
            [fi(7, 11), fi(11, 7), fi(14, 8), fi(17, 10), fi(20, 9)],
            [fi(6, 10), fi(10, 0), fi(12, 8), fi(15, 6), fi(17, 11)],
        ],
    ],
];

/// Table R802.4.1(2), No. 2 grade.
const LIVE_20_CEILING: RafterTable = [
    // Southern pine: 12", 16", 19.2", 24" on center.
    [
        [
            [fi(9, 5), fi(14, 9), fi(19, 6), fi(23, 5), fi(26, 0)],
            [fi(9, 0), fi(13, 6), fi(17, 1), fi(20, 3), fi(23, 10)],
        ],
        [
            [fi(8, 7), fi(13, 5), fi(17, 1), fi(20, 3), fi(23, 10)],
            [fi(7, 9), fi(11, 8), fi(14, 9), fi(17, 6), fi(20, 8)],
        ],
        [
            [fi(8, 1), fi(12, 3), fi(15, 7), fi(18, 6), fi(21, 9)],
            [fi(7, 1), fi(10, 8), fi(13, 6), fi(16, 0), fi(18, 10)],
        ],
        [
            [fi(7, 4), fi(11, 0), fi(13, 11), fi(16, 6), fi(19, 6)],
            [fi(6, 4), fi(9, 6), fi(12, 1), fi(14, 4), fi(16, 10)],
        ],
    ],
    // Spruce-pine-fir: 12", 16", 19.2", 24" on center.
    [
        [
            [fi(9, 5), fi(14, 9), fi(19, 6), fi(24, 10), fi(26, 0)],
            [fi(9, 5), fi(14, 4), fi(18, 2), fi(22, 3), fi(25, 9)],
        ],
        [
            [fi(8, 7), fi(13, 5), fi(17, 9), fi(22, 3), fi(25, 9)],
            [fi(8, 6), fi(12, 5), fi(15, 9), fi(19, 3), fi(22, 4)],
        ],
        [
            [fi(8, 1), fi(12, 8), fi(16, 7), fi(20, 3), fi(23, 6)],
            [fi(7, 9), fi(11, 4), fi(14, 4), fi(17, 7), fi(20, 4)],
        ],
        [
            [fi(7, 6), fi(11, 9), fi(14, 10), fi(18, 2), fi(21, 0)],
            [fi(6, 11), fi(10, 2), fi(12, 10), fi(15, 8), fi(18, 3)],
        ],
    ],
    // Douglas fir-larch: 12", 16", 19.2", 24" on center.
    [
        [
            [fi(9, 10), fi(15, 6), fi(20, 5), fi(26, 0), fi(26, 0)],
            [fi(9, 10), fi(14, 7), fi(18, 5), fi(22, 6), fi(26, 0)],
        ],
        [
            [fi(8, 11), fi(14, 1), fi(18, 5), fi(22, 6), fi(26, 0)],
            [fi(8, 7), fi(12, 7), fi(16, 0), fi(19, 6), fi(22, 7)],
        ],
        [
            [fi(8, 5), fi(13, 3), fi(16, 10), fi(20, 7), fi(23, 10)],
            [fi(7, 10), fi(11, 6), fi(14, 7), fi(17, 10), fi(20, 8)],
        ],
        [
            [fi(7, 10), fi(11, 11), fi(15, 1), fi(18, 5), fi(21, 4)],
            [fi(7, 0), fi(10, 4), fi(13, 0), fi(15, 11), fi(18, 6)],
        ],
    ],
    // Hem-fir: 12", 16", 19.2", 24" on center.
    [
        [
            [fi(9, 2), fi(14, 5), fi(19, 0), fi(24, 3), fi(26, 0)],
            [fi(9, 2), fi(14, 2), fi(17, 11), fi(21, 11), fi(25, 5)],
        ],
        [
            [fi(8, 4), fi(13, 1), fi(17, 3), fi(21, 11), fi(25, 5)],
            [fi(8, 4), fi(12, 3), fi(15, 6), fi(18, 11), fi(22, 0)],
        ],
        [
            [fi(7, 10), fi(12, 4), fi(16, 3), fi(20, 0), fi(23, 2)],
            [fi(7, 8), fi(11, 2), fi(14, 2), fi(17, 4), fi(20, 1)],
        ],
        [
            [fi(7, 3), fi(11, 5), fi(14, 8), fi(17, 10), fi(20, 9)],
            [fi(6, 10), fi(10, 0), fi(12, 8), fi(15, 6), fi(17, 11)],
        ],
    ],
];

/// Table R802.4.1(3), No. 2 grade.
const SNOW_30_NO_CEILING: RafterTable = [
    // Southern pine: 12", 16", 19.2", 24" on center.
    [
        [
            [fi(8, 7), fi(12, 11), fi(16, 4), fi(19, 5), fi(22, 10)],
            [fi(7, 8), fi(11, 7), fi(14, 8), fi(17, 4), fi(20, 5)],
        ],
        [
            [fi(7, 6), fi(11, 2), fi(14, 2), fi(16, 10), fi(19, 10)],
            [fi(6, 8), fi(10, 0), fi(12, 8), fi(15, 1), fi(17, 9)],
        ],
        [
            [fi(6, 10), fi(10, 2), fi(12, 11), fi(15, 4), fi(18, 1)],
            [fi(6, 1), fi(9, 2), fi(11, 7), fi(13, 9), fi(16, 2)],
        ],
        [
            [fi(6, 1), fi(9, 2), fi(11, 7), fi(13, 9), fi(16, 2)],
            [fi(5, 5), fi(8, 2), fi(10, 4), fi(12, 3), fi(14, 6)],
        ],
    ],
    // Spruce-pine-fir: 12", 16", 19.2", 24" on center.
    [
        [
            [fi(9, 1), fi(13, 9), fi(17, 5), fi(21, 4), fi(24, 8)],
            [fi(8, 5), fi(12, 4), fi(15, 7), fi(19, 1), fi(22, 1)],
        ],
        [
            [fi(8, 2), fi(11, 11), fi(15, 1), fi(18, 5), fi(21, 5)],
            [fi(7, 3), fi(10, 8), fi(13, 6), fi(16, 6), fi(19, 2)],
        ],
        [
            [fi(7, 5), fi(10, 11), fi(13, 9), fi(16, 10), fi(19, 6)],
            [fi(6, 8), fi(9, 9), fi(12, 4), fi(15, 1), fi(17, 6)],
        ],
        [
            [fi(6, 6), fi(9, 9), fi(12, 4), fi(15, 1), fi(17, 6)],
            [fi(5, 11), fi(8, 8), fi(11, 0), fi(13, 6), fi(15, 7)],
        ],
    ],
    // Douglas fir-larch: 12", 16", 19.2", 24" on center.
    [
        [
            [fi(9, 6), fi(14, 0), fi(17, 8), fi(21, 7), fi(25, 1)],
            [fi(8, 6), fi(12, 6), fi(15, 10), fi(19, 4), fi(22, 5)],
        ],
        [
            [fi(8, 3), fi(12, 1), fi(15, 4), fi(18, 9), fi(21, 8)],
            [fi(7, 5), fi(10, 10), fi(13, 8), fi(16, 9), fi(19, 5)],
        ],
        [
            [fi(7, 7), fi(11, 0), fi(14, 0), fi(17, 1), fi(19, 10)],
            [fi(6, 9), fi(9, 10), fi(12, 6), fi(15, 3), fi(17, 9)],
        ],
        [
            [fi(6, 9), fi(9, 10), fi(12, 6), fi(15, 3), fi(17, 9)],
            [fi(6, 0), fi(8, 10), fi(11, 2), fi(13, 8), fi(15, 10)],
        ],
    ],
    // Hem-fir: 12", 16", 19.2", 24" on center.
    [
        [
            [fi(8, 10), fi(13, 7), fi(17, 2), fi(21, 0), fi(24, 4)],
            [fi(8, 4), fi(12, 2), fi(15, 4), fi(18, 9), fi(21, 9)],
        ],
        [
            [fi(8, 0), fi(11, 9), fi(14, 11), fi(18, 2), fi(21, 1)],
            [fi(7, 2), fi(10, 6), fi(13, 4), fi(16, 3), fi(18, 10)],
        ],
        [
            [fi(7, 4), fi(10, 9), fi(13, 7), fi(16, 7), fi(19, 3)],
            [fi(6, 7), fi(9, 7), fi(12, 2), fi(14, 10), fi(17, 3)],
        ],
        [
            [fi(6, 7), fi(9, 7), fi(12, 2), fi(14, 10), fi(17, 3)],
            [fi(5, 10), fi(8, 7), fi(10, 10), fi(13, 3), fi(15, 5)],
        ],
    ],
];

/// Which rafter table a roof is checked against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Table {
    Live,
    LiveUnderCeiling,
    Snow,
}

impl Table {
    fn rows(self) -> &'static RafterTable {
        match self {
            Table::Live => &LIVE_20_NO_CEILING,
            Table::LiveUnderCeiling => &LIVE_20_CEILING,
            Table::Snow => &SNOW_30_NO_CEILING,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Table::Live => "IRC Table R802.4.1(1)",
            Table::LiveUnderCeiling => "IRC Table R802.4.1(2)",
            Table::Snow => "IRC Table R802.4.1(3)",
        }
    }
}

/// The loads and lumber a roof's rafters are checked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RafterLoads {
    pub species: FramingSpecies,
    pub ground_snow_psf: u32,
    pub dead_psf: u32,
    pub ceiling: bool,
}

impl Default for RafterLoads {
    /// What a roof that says nothing is checked for.
    fn default() -> RafterLoads {
        RafterLoads {
            species: FramingSpecies::default(),
            ground_snow_psf: DEFAULT_GROUND_SNOW_PSF,
            dead_psf: DEFAULT_ROOF_DEAD_PSF,
            ceiling: false,
        }
    }
}

impl RafterLoads {
    pub fn of(roof: &Roof) -> RafterLoads {
        RafterLoads {
            species: roof.species,
            ground_snow_psf: roof.ground_snow_psf,
            dead_psf: roof.dead_psf,
            ceiling: roof.ceiling,
        }
    }

    /// The loads a project's rafters are checked for.
    pub fn for_project(project: &Project) -> RafterLoads {
        project
            .roof
            .as_ref()
            .map(RafterLoads::of)
            .unwrap_or_default()
    }

    /// Like `southern pine No. 2, 30 psf ground snow, 10 psf dead load, no
    /// ceiling on the rafters`.
    pub fn describe(&self) -> String {
        let load = if self.ground_snow_psf <= 20 {
            "20 psf roof live load".to_string()
        } else {
            format!("{} psf ground snow", self.ground_snow_psf)
        };
        format!(
            "{} No. 2, {load}, {} psf dead load, {}",
            self.species.name(),
            self.dead_psf,
            if self.ceiling {
                "ceiling attached"
            } else {
                "no ceiling on the rafters"
            }
        )
    }
}

/// The longest clear span the rafter tables allow, with where it comes
/// from, or why the tables don't cover the case.
pub fn rafter_span(
    loads: &RafterLoads,
    size: Nominal,
    oc: Length,
) -> Result<(Length, String), String> {
    let table = match (loads.ground_snow_psf, loads.ceiling) {
        (0..=20, false) => Table::Live,
        (0..=20, true) => Table::LiveUnderCeiling,
        (21..=30, false) => Table::Snow,
        (21..=30, true) => {
            return Err(
                "Sticks doesn't carry Table R802.4.1(4) for a ceiling under a snow roof yet; size these by hand"
                    .to_string(),
            )
        }
        (snow, _) => {
            return Err(format!(
                "Sticks only carries rafter tables up to 30 psf of ground snow, not {snow}"
            ))
        }
    };
    if size != Nominal::new(2, size.width) {
        return Err(format!("the rafter tables cover 2x lumber, not {size}"));
    }
    let col = match size.width {
        4 => 0,
        6 => 1,
        8 => 2,
        10 => 3,
        12 => 4,
        _ => return Err(format!("the rafter tables cover 2x4 to 2x12, not {size}")),
    };
    let spacing = SPACINGS_16
        .iter()
        .position(|&s| oc.sixteenths() <= s)
        .ok_or_else(|| format!("the rafter tables stop at 24\" on center, not {oc:#}"))?;
    let dead = match loads.dead_psf {
        0..=10 => 0,
        11..=20 => 1,
        d => {
            return Err(format!(
                "the rafter tables stop at 20 psf of dead load, not {d}"
            ))
        }
    };
    let species = match loads.species {
        FramingSpecies::SouthernPine => 0,
        FramingSpecies::SprucePineFir => 1,
        FramingSpecies::DouglasFirLarch => 2,
        FramingSpecies::HemFir => 3,
    };
    let span = table.rows()[species][spacing][dead][col];
    let source = format!(
        "{}, {} No. 2 {size} at {} o.c., {} psf dead",
        table.name(),
        loads.species.name(),
        SPACING_NAMES[spacing],
        if dead == 0 { 10 } else { 20 }
    );
    Ok((span, source))
}

/// The smallest rafter that spans `span` at `oc`, if any 2x does.
pub fn smallest_rafter(loads: &RafterLoads, span: Length, oc: Length) -> Option<Nominal> {
    [4, 6, 8, 10, 12]
        .into_iter()
        .map(|w| Nominal::new(2, w))
        .find(|&n| rafter_span(loads, n, oc).is_ok_and(|(a, _)| span <= a))
}

/// What a set of rafters' span check is called, like `Porch rafter span`.
pub fn rafter_item(d: &RafterDetail) -> String {
    format!("{}{} span", d.name[..1].to_uppercase(), &d.name[1..])
}

/// Checks one set of rafters cut alike: the span from each support to the
/// next against the table.
pub fn check_rafter(
    d: &RafterDetail,
    loads: &RafterLoads,
    oc: Length,
    members: Vec<String>,
) -> CodeCheck {
    let item = rafter_item(d);
    let actual = format!("{} {} at {oc:#} o.c., {} clear", d.count, d.size, d.span);
    let mut c = match rafter_span(loads, d.size, oc) {
        Ok((allowed, source)) => {
            let ok = d.span <= allowed;
            let mut allowed = format!("{allowed}");
            if !ok {
                match smallest_rafter(loads, d.span, oc) {
                    Some(n) => allowed.push_str(&format!("; {n} spans it")),
                    None => allowed.push_str("; no 2x spans it, add a support"),
                }
            }
            check(&item, actual, allowed, ok, source)
        }
        Err(why) => check(&item, actual, "not in the tables".to_string(), false, why),
    };
    c.members = members;
    c
}

/// Notes a failed check on each member it covers.
pub fn mark(members: &mut [Placed], c: &CodeCheck) {
    if c.ok {
        return;
    }
    let note = format!(
        "{}: {}; allowed {} ({})",
        c.item, c.actual, c.allowed, c.source
    );
    for p in members
        .iter_mut()
        .filter(|p| c.members.contains(&p.member.label))
    {
        p.fails.push(note.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loads(snow: u32) -> RafterLoads {
        RafterLoads {
            species: FramingSpecies::SouthernPine,
            ground_snow_psf: snow,
            dead_psf: 10,
            ceiling: false,
        }
    }

    #[test]
    fn reads_the_snow_table() {
        let l = loads(25);
        let (s, src) = rafter_span(&l, Nominal::new(2, 6), Length::inches(16)).unwrap();
        assert_eq!(s, fi(11, 2));
        assert!(src.contains("R802.4.1(3)"));
        let (s, _) = rafter_span(&l, Nominal::new(2, 8), Length::inches(16)).unwrap();
        assert_eq!(s, fi(14, 2));
        // Spruce-pine-fir 2x8 at 24", which reuven.tools quotes too.
        let spf = RafterLoads {
            species: FramingSpecies::SprucePineFir,
            ..l
        };
        let (s, _) = rafter_span(&spf, Nominal::new(2, 8), Length::inches(24)).unwrap();
        assert_eq!(s, fi(12, 4));
    }

    #[test]
    fn light_snow_uses_the_live_load_tables() {
        let (s, src) = rafter_span(&loads(20), Nominal::new(2, 6), Length::inches(16)).unwrap();
        assert_eq!(s, fi(13, 6));
        assert!(src.contains("R802.4.1(1)"));
    }

    #[test]
    fn between_columns_takes_the_safer_one() {
        // 18" on center reads the 19.2" column; 15 psf dead reads the 20.
        let l = RafterLoads {
            dead_psf: 15,
            ..loads(30)
        };
        let (s, _) = rafter_span(&l, Nominal::new(2, 8), Length::inches(18)).unwrap();
        assert_eq!(s, fi(11, 7));
    }

    #[test]
    fn says_what_it_doesnt_cover() {
        let ceiling = RafterLoads {
            ceiling: true,
            ..loads(30)
        };
        assert!(rafter_span(&ceiling, Nominal::new(2, 6), Length::inches(16)).is_err());
        assert!(rafter_span(&loads(40), Nominal::new(2, 6), Length::inches(16)).is_err());
        assert!(rafter_span(&loads(30), Nominal::new(4, 6), Length::inches(16)).is_err());
        assert!(rafter_span(&loads(30), Nominal::new(2, 6), Length::inches(32)).is_err());
    }

    #[test]
    fn suggests_the_smallest_rafter_that_spans() {
        let span = Length::inches(13 * 12 + 5);
        assert_eq!(
            smallest_rafter(&loads(25), span, Length::inches(16)),
            Some(Nominal::new(2, 8))
        );
    }
}
