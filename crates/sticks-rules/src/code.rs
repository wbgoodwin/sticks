//! Deck rules: the prescriptive tables a deck's sizes are checked against,
//! and the checks themselves.
//!
//! Two sets of rules are carried:
//!
//! - The International Residential Code's deck tables (R507, 2021), for 40
//!   psf of live load and 10 psf of dead load, No. 2 lumber or better, wet
//!   service. Joist spans and cantilevers are Table R507.6. Beam spans are
//!   Table R507.5 for joists framing in from one side; where two published
//!   copies differ the shorter span is kept. Sources: AWC's DCA 6 deck
//!   guide, the Livonia, MI deck guide and deckmath.com's copy of the 2021
//!   tables. Footings are sized from the same loads over the soil's bearing
//!   pressure, the basis of Table R507.3.1.
//! - Fairfax County, Virginia's Typical Deck Details, version 2021.0 (dated
//!   1/5/2024, 2021 Virginia Residential Code): its Tables 1 to 5 for
//!   decking, joists, beams, footings and posts, and its rules for footing
//!   depth, post size and bracing.
//!
//! Values between table columns aren't interpolated: a lookup takes the
//! column that's never less safe. A design off the tables is reported as
//! not covered rather than guessed.

use serde::Serialize;
use sticks_core::{
    BeamStyle, Deck, DeckCode, DeckSupport, DeckingMaterial, Length, Nominal, Species,
};

/// Decks higher than this above the ground need a guard (IRC R312.1.1;
/// Fairfax Section 9).
pub const GUARD_ABOVE: Length = Length::inches(30);

/// Dead load both sets of tables assume, pounds per square foot.
pub const DEAD_PSF: u32 = 10;

/// One requirement checked against the rules.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CodeCheck {
    /// What's checked, like `Joist span`.
    pub item: String,
    /// What the design has.
    pub actual: String,
    /// What the rules allow or require.
    pub allowed: String,
    pub ok: bool,
    /// Where the limit comes from, like `R507.6, southern pine 2x8 at 16" o.c.`.
    pub source: String,
}

fn check(item: &str, actual: String, allowed: String, ok: bool, source: String) -> CodeCheck {
    CodeCheck {
        item: item.to_string(),
        actual,
        allowed,
        ok,
        source,
    }
}

const fn fi(feet: i64, inches: i64) -> Length {
    Length::from_sixteenths((feet * 12 + inches) * 16)
}

/// Joist spacings the joist tables have columns for.
const SPACINGS: [Length; 3] = [Length::inches(12), Length::inches(16), Length::inches(24)];

/// Joist table row: (joist depth, [span at 12", 16", 24"], [cantilever at
/// 12", 16", 24"]).
type JoistRow = (u8, [Length; 3], [Length; 3]);

const SOUTHERN_PINE_JOISTS: [JoistRow; 4] = [
    (
        6,
        [fi(9, 11), fi(9, 0), fi(7, 7)],
        [fi(1, 0), fi(1, 1), fi(1, 3)],
    ),
    (
        8,
        [fi(13, 1), fi(11, 10), fi(9, 8)],
        [fi(1, 10), fi(2, 0), fi(2, 4)],
    ),
    (
        10,
        [fi(16, 2), fi(14, 0), fi(11, 5)],
        [fi(3, 1), fi(3, 5), fi(2, 10)],
    ),
    (
        12,
        [fi(18, 0), fi(16, 6), fi(13, 6)],
        [fi(4, 6), fi(4, 2), fi(3, 4)],
    ),
];

const DOUGLAS_FIR_JOISTS: [JoistRow; 4] = [
    (
        6,
        [fi(9, 6), fi(8, 4), fi(6, 10)],
        [fi(0, 11), fi(1, 0), fi(1, 2)],
    ),
    (
        8,
        [fi(12, 6), fi(11, 1), fi(9, 1)],
        [fi(1, 8), fi(1, 10), fi(2, 2)],
    ),
    (
        10,
        [fi(15, 8), fi(13, 7), fi(11, 1)],
        [fi(2, 10), fi(3, 2), fi(2, 9)],
    ),
    (
        12,
        [fi(18, 0), fi(15, 9), fi(12, 10)],
        [fi(4, 4), fi(3, 11), fi(3, 3)],
    ),
];

const REDWOOD_JOISTS: [JoistRow; 4] = [
    (
        6,
        [fi(8, 10), fi(8, 0), fi(6, 10)],
        [fi(0, 9), fi(0, 10), fi(0, 11)],
    ),
    (
        8,
        [fi(11, 8), fi(10, 7), fi(8, 8)],
        [fi(1, 5), fi(1, 7), fi(1, 9)],
    ),
    (
        10,
        [fi(14, 11), fi(13, 0), fi(10, 7)],
        [fi(2, 5), fi(2, 7), fi(2, 8)],
    ),
    (
        12,
        [fi(17, 5), fi(15, 1), fi(12, 4)],
        [fi(3, 7), fi(3, 9), fi(3, 1)],
    ),
];

/// Where a joist size and spacing land in a joist table: the row and the
/// column, spacings between columns taking the wider one.
fn joist_row(species: Species, joist: Nominal, oc: Length) -> Option<(&'static JoistRow, usize)> {
    if joist != Nominal::new(2, joist.width) {
        return None;
    }
    let col = SPACINGS.iter().position(|&s| oc <= s)?;
    let rows: &'static [JoistRow; 4] = match species {
        Species::SouthernPine => &SOUTHERN_PINE_JOISTS,
        Species::DouglasFir => &DOUGLAS_FIR_JOISTS,
        Species::Redwood => &REDWOOD_JOISTS,
    };
    rows.iter().find(|r| r.0 == joist.width).map(|r| (r, col))
}

/// The table's column spacing for `oc`, like `16" o.c.`.
fn spacing_note(oc: Length) -> String {
    SPACINGS
        .iter()
        .find(|&&s| oc <= s)
        .map(|s| format!("{s:#} o.c."))
        .unwrap_or_else(|| format!("{oc:#} o.c."))
}

/// The longest joist span, bearing center to bearing center, the joist
/// table allows. Fairfax's Table 2 spans are the IRC's southern pine row.
pub fn joist_span(species: Species, joist: Nominal, oc: Length) -> Option<Length> {
    joist_row(species, joist, oc).map(|(r, c)| r.1[c])
}

/// Fairfax Table 2: allowable joist overhang by joist span, 4' to 18'.
/// `None` where the table says not permitted.
const FAIRFAX_OVERHANGS: [(u8, [Option<Length>; 8]); 4] = [
    (
        6,
        [
            Some(fi(1, 0)),
            Some(fi(1, 6)),
            Some(fi(1, 5)),
            None,
            None,
            None,
            None,
            None,
        ],
    ),
    (
        8,
        [
            Some(fi(1, 0)),
            Some(fi(1, 6)),
            Some(fi(2, 0)),
            Some(fi(2, 6)),
            Some(fi(2, 3)),
            None,
            None,
            None,
        ],
    ),
    (
        10,
        [
            Some(fi(1, 0)),
            Some(fi(1, 6)),
            Some(fi(2, 0)),
            Some(fi(2, 6)),
            Some(fi(3, 0)),
            Some(fi(3, 4)),
            Some(fi(3, 4)),
            None,
        ],
    ),
    (
        12,
        [
            Some(fi(1, 0)),
            Some(fi(1, 6)),
            Some(fi(2, 0)),
            Some(fi(2, 6)),
            Some(fi(3, 0)),
            Some(fi(3, 6)),
            Some(fi(4, 0)),
            Some(fi(4, 1)),
        ],
    ),
];

/// The longest overhang past a beam for a joist with `back_span` behind it,
/// never more than a quarter of the back span.
pub fn joist_overhang(
    code: DeckCode,
    species: Species,
    joist: Nominal,
    oc: Length,
    back_span: Length,
) -> Option<Length> {
    let quarter = back_span.mul_div(1, 4);
    match code {
        DeckCode::Irc2021 => joist_row(species, joist, oc).map(|(r, c)| r.2[c].min(quarter)),
        DeckCode::Fairfax2021 => {
            if joist != Nominal::new(2, joist.width) {
                return None;
            }
            let row = FAIRFAX_OVERHANGS.iter().find(|r| r.0 == joist.width)?;
            // The columns on either side of the span; the lesser of the two.
            let feet = back_span.as_inches_f64() / 12.0;
            let lo = (((feet - 4.0) / 2.0).floor().max(0.0) as usize).min(7);
            let hi = (((feet - 4.0) / 2.0).ceil().max(0.0) as usize).min(7);
            if feet > 18.0 {
                return Some(Length::ZERO);
            }
            let a = row.1[lo].unwrap_or(Length::ZERO);
            let b = row.1[hi].unwrap_or(Length::ZERO);
            Some(a.min(b).min(quarter))
        }
    }
}

/// Effective joist spans, in feet, the IRC beam table has columns for; also
/// Fairfax's beam span columns.
const SPAN_COLUMNS: [i64; 7] = [6, 8, 10, 12, 14, 16, 18];

/// IRC Table R507.5, joists from one side: (plies, board thickness, board
/// depth, spans by effective joist span). A solid 4x beam is one ply.
type BeamRow = (u8, u8, u8, [Length; 7]);

const SOUTHERN_PINE_BEAMS: [BeamRow; 8] = [
    (
        2,
        2,
        6,
        [
            fi(6, 11),
            fi(5, 11),
            fi(5, 4),
            fi(4, 10),
            fi(4, 6),
            fi(4, 3),
            fi(4, 0),
        ],
    ),
    (
        2,
        2,
        8,
        [
            fi(8, 9),
            fi(7, 7),
            fi(6, 9),
            fi(6, 2),
            fi(5, 9),
            fi(5, 4),
            fi(5, 0),
        ],
    ),
    (
        2,
        2,
        10,
        [
            fi(10, 4),
            fi(9, 0),
            fi(8, 0),
            fi(7, 4),
            fi(6, 9),
            fi(6, 4),
            fi(6, 0),
        ],
    ),
    (
        2,
        2,
        12,
        [
            fi(12, 2),
            fi(10, 7),
            fi(9, 5),
            fi(8, 7),
            fi(8, 0),
            fi(7, 5),
            fi(7, 0),
        ],
    ),
    (
        3,
        2,
        6,
        [
            fi(8, 2),
            fi(7, 5),
            fi(6, 8),
            fi(6, 1),
            fi(5, 8),
            fi(5, 3),
            fi(4, 11),
        ],
    ),
    (
        3,
        2,
        8,
        [
            fi(10, 10),
            fi(9, 6),
            fi(8, 6),
            fi(7, 9),
            fi(7, 2),
            fi(6, 8),
            fi(6, 4),
        ],
    ),
    (
        3,
        2,
        10,
        [
            fi(13, 0),
            fi(11, 2),
            fi(10, 0),
            fi(9, 2),
            fi(8, 6),
            fi(7, 11),
            fi(7, 6),
        ],
    ),
    (
        3,
        2,
        12,
        [
            fi(15, 3),
            fi(13, 3),
            fi(11, 10),
            fi(10, 9),
            fi(10, 0),
            fi(9, 4),
            fi(8, 10),
        ],
    ),
];

const DOUGLAS_FIR_BEAMS: [BeamRow; 12] = [
    (
        2,
        2,
        6,
        [
            fi(5, 5),
            fi(4, 8),
            fi(4, 2),
            fi(3, 10),
            fi(3, 6),
            fi(3, 1),
            fi(2, 9),
        ],
    ),
    (
        2,
        2,
        8,
        [
            fi(6, 10),
            fi(5, 11),
            fi(5, 4),
            fi(4, 10),
            fi(4, 6),
            fi(4, 1),
            fi(3, 8),
        ],
    ),
    (
        2,
        2,
        10,
        [
            fi(8, 4),
            fi(7, 3),
            fi(6, 6),
            fi(5, 11),
            fi(5, 6),
            fi(5, 1),
            fi(4, 8),
        ],
    ),
    (
        2,
        2,
        12,
        [
            fi(9, 8),
            fi(8, 5),
            fi(7, 6),
            fi(6, 10),
            fi(6, 4),
            fi(5, 11),
            fi(5, 7),
        ],
    ),
    (
        1,
        4,
        6,
        [
            fi(6, 5),
            fi(5, 6),
            fi(4, 11),
            fi(4, 6),
            fi(4, 2),
            fi(3, 11),
            fi(3, 8),
        ],
    ),
    (
        1,
        4,
        8,
        [
            fi(8, 5),
            fi(7, 3),
            fi(6, 6),
            fi(5, 11),
            fi(5, 6),
            fi(5, 2),
            fi(4, 10),
        ],
    ),
    (
        1,
        4,
        10,
        [
            fi(9, 11),
            fi(8, 7),
            fi(7, 8),
            fi(7, 0),
            fi(6, 6),
            fi(6, 1),
            fi(5, 8),
        ],
    ),
    (
        1,
        4,
        12,
        [
            fi(11, 5),
            fi(9, 11),
            fi(8, 10),
            fi(8, 1),
            fi(7, 6),
            fi(7, 0),
            fi(6, 7),
        ],
    ),
    (
        3,
        2,
        6,
        [
            fi(7, 4),
            fi(6, 8),
            fi(6, 0),
            fi(5, 6),
            fi(5, 1),
            fi(4, 9),
            fi(4, 6),
        ],
    ),
    (
        3,
        2,
        8,
        [
            fi(9, 8),
            fi(8, 6),
            fi(7, 7),
            fi(6, 11),
            fi(6, 5),
            fi(6, 0),
            fi(5, 8),
        ],
    ),
    (
        3,
        2,
        10,
        [
            fi(12, 0),
            fi(10, 5),
            fi(9, 4),
            fi(8, 6),
            fi(7, 10),
            fi(7, 4),
            fi(6, 11),
        ],
    ),
    (
        3,
        2,
        12,
        [
            fi(13, 11),
            fi(12, 1),
            fi(10, 9),
            fi(9, 10),
            fi(9, 1),
            fi(8, 6),
            fi(8, 1),
        ],
    ),
];

const REDWOOD_BEAMS: [BeamRow; 4] = [
    (
        3,
        2,
        6,
        [
            fi(7, 4),
            fi(6, 8),
            fi(6, 0),
            fi(5, 6),
            fi(5, 1),
            fi(4, 9),
            fi(4, 6),
        ],
    ),
    (
        3,
        2,
        8,
        [
            fi(9, 8),
            fi(8, 6),
            fi(7, 7),
            fi(6, 11),
            fi(6, 5),
            fi(6, 0),
            fi(5, 8),
        ],
    ),
    (
        3,
        2,
        10,
        [
            fi(12, 0),
            fi(10, 5),
            fi(9, 4),
            fi(8, 6),
            fi(7, 10),
            fi(7, 4),
            fi(6, 11),
        ],
    ),
    (
        3,
        2,
        12,
        [
            fi(13, 11),
            fi(12, 1),
            fi(10, 9),
            fi(9, 10),
            fi(9, 1),
            fi(8, 6),
            fi(8, 1),
        ],
    ),
];

/// IRC: a beam's longest span between supports from Table R507.5, carrying
/// joists with `effective` span, and the table column it was read from.
pub fn irc_beam_span(
    species: Species,
    board: Nominal,
    plies: u8,
    effective: Length,
) -> Option<(Length, Length)> {
    let rows: &[BeamRow] = match species {
        Species::SouthernPine => &SOUTHERN_PINE_BEAMS,
        Species::DouglasFir => &DOUGLAS_FIR_BEAMS,
        Species::Redwood => &REDWOOD_BEAMS,
    };
    let row = rows
        .iter()
        .find(|r| r.0 == plies && board == Nominal::new(r.1, r.2))?;
    let col = SPAN_COLUMNS
        .iter()
        .position(|&f| effective <= Length::feet(f))?;
    Some((row.3[col], Length::feet(SPAN_COLUMNS[col])))
}

/// Fairfax Table 3: the smallest beams, as (plies, 2x depth), for beam
/// influence widths of 2' to 18' (rows) and beam spans up to 6' to 18'
/// (columns). Empty where the table's shading prohibits the design.
const FAIRFAX_BEAMS: [[&[(u8, u8)]; 7]; 17] = {
    const N: &[(u8, u8)] = &[];
    const A6: &[(u8, u8)] = &[(2, 6)];
    const A8: &[(u8, u8)] = &[(2, 8)];
    const A10: &[(u8, u8)] = &[(2, 10)];
    const A12: &[(u8, u8)] = &[(2, 12)];
    const B10: &[(u8, u8)] = &[(3, 10)];
    const B12: &[(u8, u8)] = &[(3, 12)];
    const C8: &[(u8, u8)] = &[(2, 8), (3, 6)];
    const C10: &[(u8, u8)] = &[(2, 10), (3, 8)];
    const C12: &[(u8, u8)] = &[(2, 12), (3, 10)];
    [
        [A6, A6, A6, A8, A10, A12, B10],
        [A6, A6, A6, A10, A10, B10, B12],
        [A6, A6, C8, A10, A12, B12, N],
        [A6, A6, C8, C10, B12, N, N],
        [A6, C8, C10, C12, N, N, N],
        [C8, C10, C12, B12, N, N, N],
        [C8, C10, C12, N, N, N, N],
        [C8, C12, B10, N, N, N, N],
        [C8, C12, B10, N, N, N, N],
        [C8, C12, N, N, N, N, N],
        [C8, C12, N, N, N, N, N],
        [C10, C12, N, N, N, N, N],
        [C10, C12, N, N, N, N, N],
        [C10, B12, N, N, N, N, N],
        [C10, B12, N, N, N, N, N],
        [C12, B12, N, N, N, N, N],
        [C12, N, N, N, N, N, N],
    ]
};

/// Fairfax Table 4: footing type and the tallest post, in feet, by the same
/// rows and columns as Table 3.
const FAIRFAX_FOOTINGS: [[Option<(char, u8)>; 7]; 17] = {
    const fn f(t: char, h: u8) -> Option<(char, u8)> {
        Some((t, h))
    }
    const N: Option<(char, u8)> = None;
    [
        [
            f('A', 14),
            f('A', 14),
            f('A', 14),
            f('A', 14),
            f('A', 14),
            f('B', 14),
            f('B', 14),
        ],
        [
            f('A', 14),
            f('A', 14),
            f('B', 14),
            f('B', 14),
            f('B', 14),
            f('B', 14),
            f('C', 14),
        ],
        [
            f('A', 14),
            f('B', 14),
            f('B', 14),
            f('B', 14),
            f('C', 14),
            f('D', 14),
            N,
        ],
        [
            f('B', 14),
            f('B', 14),
            f('C', 14),
            f('D', 14),
            f('E', 14),
            N,
            N,
        ],
        [f('B', 14), f('B', 14), f('D', 14), f('E', 14), N, N, N],
        [f('B', 14), f('C', 14), f('E', 14), f('E', 13), N, N, N],
        [f('B', 14), f('C', 14), f('E', 13), N, N, N, N],
        [f('C', 14), f('D', 14), f('E', 12), N, N, N, N],
        [f('D', 14), f('E', 13), f('F', 11), N, N, N, N],
        [f('D', 14), f('E', 13), N, N, N, N, N],
        [f('E', 14), f('F', 12), N, N, N, N, N],
        [f('E', 14), f('F', 11), N, N, N, N, N],
        [f('E', 13), f('F', 10), N, N, N, N, N],
        [f('F', 12), f('G', 9), N, N, N, N, N],
        [f('F', 12), f('H', 9), N, N, N, N, N],
        [f('G', 11), f('H', 9), N, N, N, N, N],
        [f('G', 11), N, N, N, N, N, N],
    ]
};

/// Fairfax Table 5: footing types A to H as (square side, round diameter,
/// thickness), in inches.
const FAIRFAX_FOOTING_SIZES: [(char, i64, i64, i64); 8] = [
    ('A', 12, 14, 6),
    ('B', 14, 16, 6),
    ('C', 16, 18, 6),
    ('D', 18, 20, 6),
    ('E', 20, 22, 8),
    ('F', 22, 24, 8),
    ('G', 24, 27, 9),
    ('H', 26, 29, 10),
];

/// The Fairfax table cell for a beam's influence width and span: the row
/// for the next whole foot of width up, the column for the next span up.
fn fairfax_cell(influence: Length, span: Length) -> Option<(usize, usize, i64, i64)> {
    let width_ft = ((influence.sixteenths() + 191) / 192).max(2);
    if width_ft > 18 {
        return None;
    }
    let col = SPAN_COLUMNS.iter().position(|&f| span <= Length::feet(f))?;
    Some(((width_ft - 2) as usize, col, width_ft, SPAN_COLUMNS[col]))
}

/// Fairfax: whether a beam of `plies` 2x`depth` is at least one of the
/// table's choices for its influence width and span.
fn fairfax_beam_ok(board: Nominal, plies: u8, influence: Length, span: Length) -> Option<bool> {
    let (row, col, ..) = fairfax_cell(influence, span)?;
    let choices = FAIRFAX_BEAMS[row][col];
    if choices.is_empty() {
        return None;
    }
    if board != Nominal::new(2, board.width) {
        return Some(false);
    }
    Some(choices.iter().any(|&(p, d)| plies >= p && board.width >= d))
}

/// The longest span between supports the rules allow for a beam, so a deck
/// left to choose its post spacing gets as few footings as it can.
pub fn longest_beam_span(deck: &Deck, influence: Length, effective: Length) -> Option<Length> {
    match deck.code {
        DeckCode::Irc2021 => {
            irc_beam_span(deck.species, deck.beam, deck.beam_plies, effective).map(|(s, _)| s)
        }
        DeckCode::Fairfax2021 => {
            SPAN_COLUMNS
                .iter()
                .rev()
                .map(|&f| Length::feet(f))
                .find(|&span| {
                    fairfax_beam_ok(deck.beam, deck.beam_plies, influence, span) == Some(true)
                        && fairfax_cell(influence, span)
                            .and_then(|(r, c, ..)| FAIRFAX_FOOTINGS[r][c])
                            .is_some()
                })
        }
    }
}

/// How a beam is written in the tables, like `(2) 2x8` or `4x6`.
pub fn beam_name(board: Nominal, plies: u8) -> String {
    if plies == 1 {
        board.to_string()
    } else {
        format!("({plies}) {board}")
    }
}

/// The IRC's effective joist span for a beam, for its beam table: the span
/// of joists from one side that would put the same load on it. A joist
/// running `back` to the next beam and `overhang` past this one loads it
/// like a plain span of `(back + overhang)^2 / back`.
pub fn effective_joist_span(back: Length, overhang: Length) -> Length {
    let total = back + overhang;
    total.mul_div(total.sixteenths(), back.sixteenths().max(1))
}

/// The widest joist spacing decking may span with boards laid square across
/// the joists (IRC Table R507.7, Fairfax Table 1). `None` for composite
/// boards, whose maker sets the spacing.
pub fn decking_spacing(decking: Nominal, material: DeckingMaterial) -> Option<Length> {
    match material {
        DeckingMaterial::Composite => None,
        DeckingMaterial::Wood if decking.actual_thickness() >= Length::from_sixteenths(24) => {
            Some(Length::inches(24))
        }
        DeckingMaterial::Wood => Some(Length::inches(16)),
    }
}

/// The tallest post IRC Table R507.4 allows, top of footing to underside of
/// beam, for the smallest decks it covers.
pub fn irc_post_height(post: Nominal) -> Option<Length> {
    [
        (Nominal::new(4, 4), Length::feet(8)),
        (Nominal::new(4, 6), Length::feet(8)),
        (Nominal::new(6, 6), Length::feet(14)),
        (Nominal::new(8, 8), Length::feet(14)),
    ]
    .into_iter()
    .find(|(p, _)| *p == post)
    .map(|(_, h)| h)
}

/// Fairfax allows 4x4 posts only this tall; taller posts are 6x6.
pub const FAIRFAX_4X4_POST: Length = Length::inches(30);

/// Fairfax: posts taller than this on a freestanding deck need lateral
/// bracing (Table 8, methods 2 and 3).
pub const FAIRFAX_BRACE_ABOVE: Length = Length::inches(30);

/// A concrete footing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Footing {
    /// The type letter, where the rules name footings by type.
    pub kind: Option<char>,
    /// Area of deck it carries, in square feet.
    pub area_sqft: u32,
    /// Load on it from that area, in pounds.
    pub load_lb: u32,
    /// Side of a square footing.
    pub side: Length,
    /// Diameter of a round footing.
    pub diameter: Length,
    pub thickness: Length,
    /// How far below the ground its bottom is.
    pub depth: Length,
}

/// IRC: sizes a footing carrying `area_sq_in` of deck at `live_psf` plus
/// the dead load, on soil that bears `soil_psf`: the bearing area the load
/// needs, its own weight included, rounded up to the inch, no smaller than
/// 12" square by 6" thick, and thick enough to spread the load out from a
/// post `post_width` wide at 45 degrees.
pub fn irc_footing(area_sq_in: i64, live_psf: u32, soil_psf: u32, post_width: Length) -> Footing {
    let area_sqft = area_sq_in as f64 / 144.0;
    let load = area_sqft * f64::from(live_psf + DEAD_PSF);
    let mut side_in: f64 = 12.0;
    let mut thick_in: f64 = 6.0;
    for _ in 0..8 {
        // Concrete weighs about 150 pounds a cubic foot.
        let own = 150.0 * (side_in / 12.0).powi(2) * thick_in / 12.0;
        let needed = ((load + own) / f64::from(soil_psf.max(1))).sqrt() * 12.0;
        side_in = needed.ceil().max(12.0);
        thick_in = ((side_in - post_width.as_inches_f64()) / 2.0)
            .ceil()
            .max(6.0);
    }
    let diameter = (side_in * 2.0 / std::f64::consts::PI.sqrt()).ceil();
    Footing {
        kind: None,
        area_sqft: area_sqft.ceil() as u32,
        load_lb: load.ceil() as u32,
        side: Length::inches(side_in as i64),
        diameter: Length::inches(diameter as i64),
        thickness: Length::inches(thick_in as i64),
        depth: Length::inches(12),
    }
}

/// One beam line as the checks see it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BeamLoad {
    /// How the drawings name it, like `front beam`.
    pub name: String,
    /// Fairfax's beam influence width: half the joist span on each side it
    /// carries, plus any overhang past it.
    pub influence: Length,
    /// The IRC's effective joist span for it.
    pub effective: Length,
    /// Longest distance between support centers.
    pub span: Length,
    /// How far the beam runs past its end supports' centers.
    pub end_overhang: Length,
    /// Each support's share of the beam, along it, for its footing.
    pub shares: Vec<Length>,
}

/// What the deck framer worked out, for the checks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DeckLayout {
    pub oc: Length,
    /// Joist spans between bearing centers, front to back.
    pub joist_spans: Vec<Length>,
    /// Joist overhangs past the front and back beams' centers.
    pub overhangs: [Length; 2],
    pub beams: Vec<BeamLoad>,
    /// The tallest post, top of footing to underside of beam, when the
    /// beams sit on posts.
    pub post_height: Option<Length>,
}

/// The footing under each support of `beam`, in order along it.
pub fn footings(deck: &Deck, beam: &BeamLoad, post_width: Length) -> Vec<Footing> {
    let out: Vec<Footing> = beam
        .shares
        .iter()
        .map(|&share| {
            let area = beam.influence.sixteenths() * share.sixteenths() / 256;
            match deck.code {
                DeckCode::Irc2021 => {
                    let mut f = irc_footing(area, deck.live_psf, deck.soil_psf, post_width);
                    f.depth = f.depth.max(deck.frost_depth.unwrap_or(Length::ZERO));
                    f
                }
                DeckCode::Fairfax2021 => {
                    let kind = fairfax_cell(beam.influence, beam.span)
                        .and_then(|(r, c, ..)| FAIRFAX_FOOTINGS[r][c])
                        .map(|(k, _)| k);
                    let size = FAIRFAX_FOOTING_SIZES
                        .iter()
                        .find(|s| Some(s.0) == kind)
                        .copied()
                        .unwrap_or(('?', 0, 0, 0));
                    let area_sqft = (area as f64 / 144.0).ceil() as u32;
                    Footing {
                        kind,
                        area_sqft,
                        load_lb: area_sqft * (deck.live_psf + DEAD_PSF),
                        side: Length::inches(size.1),
                        diameter: Length::inches(size.2),
                        thickness: Length::inches(size.3),
                        depth: Length::inches(24).max(deck.frost_depth.unwrap_or(Length::ZERO)),
                    }
                }
            }
        })
        .collect();
    out
}

/// Checks `deck`, framed as `layout`, against its rules.
pub fn check_deck(deck: &Deck, layout: &DeckLayout) -> Vec<CodeCheck> {
    let fairfax = deck.code == DeckCode::Fairfax2021;
    let mut out = Vec::new();
    let joist = deck.joist;
    let oc = layout.oc;
    let joist_ref = |section: &str| {
        if fairfax {
            format!("Fairfax {section}, {joist} at {}", spacing_note(oc))
        } else {
            format!(
                "IRC {section}, {} {joist} at {}",
                deck.species.name(),
                spacing_note(oc)
            )
        }
    };

    if fairfax {
        out.push(check(
            "Lumber",
            format!("{}, treated", deck.species.name()),
            "preservative-treated southern pine, No. 2 or better".to_string(),
            deck.species == Species::SouthernPine,
            "Fairfax Section 2".to_string(),
        ));
    }

    // Joists.
    let longest = layout.joist_spans.iter().copied().max().unwrap_or_default();
    let allowed = joist_span(deck.species, joist, oc);
    out.push(check(
        "Joist span",
        format!("{longest}"),
        allowed.map_or("not in table".to_string(), |a| a.to_string()),
        allowed.is_some_and(|a| longest <= a),
        joist_ref(if fairfax { "Table 2" } else { "Table R507.6" }),
    ));
    for (i, &over) in layout.overhangs.iter().enumerate() {
        if over <= Length::ZERO {
            continue;
        }
        let back = if i == 0 {
            layout.joist_spans.first()
        } else {
            layout.joist_spans.last()
        }
        .copied()
        .unwrap_or_default();
        let allowed = joist_overhang(deck.code, deck.species, joist, oc, back);
        out.push(check(
            if i == 0 {
                "Joist overhang, front"
            } else {
                "Joist overhang, back"
            },
            format!("{over}"),
            allowed.map_or("not in table".to_string(), |a| {
                if a == Length::ZERO {
                    "not permitted".to_string()
                } else {
                    format!("{a} (and no more than 1/4 of the {back} span)")
                }
            }),
            allowed.is_some_and(|a| over <= a),
            joist_ref(if fairfax { "Table 2" } else { "Table R507.6" }),
        ));
    }
    if deck.beam_style == BeamStyle::Flush {
        out.push(check(
            "Flush beam depth",
            beam_name(deck.beam, deck.beam_plies),
            format!("at least as deep as the {joist} joists"),
            deck.beam.actual_width() >= joist.actual_width(),
            if fairfax {
                "Fairfax Section 4".to_string()
            } else {
                "IRC R507.5 table note".to_string()
            },
        ));
    }

    // Beams.
    for beam in &layout.beams {
        let name = beam_name(deck.beam, deck.beam_plies);
        if fairfax {
            let cell = fairfax_cell(beam.influence, beam.span);
            let ok = fairfax_beam_ok(deck.beam, deck.beam_plies, beam.influence, beam.span);
            let allowed = match cell {
                Some((r, c, ..)) if !FAIRFAX_BEAMS[r][c].is_empty() => FAIRFAX_BEAMS[r][c]
                    .iter()
                    .map(|&(p, d)| beam_name(Nominal::new(2, d), p))
                    .collect::<Vec<_>>()
                    .join(" or "),
                _ => "not permitted; shorten the span between posts".to_string(),
            };
            let (w, s) = cell.map_or((0, 0), |(_, _, w, s)| (w, s));
            out.push(check(
                &format!("Beam size, {}", beam.name),
                format!(
                    "{name} spanning {} with {} of joists",
                    beam.span, beam.influence
                ),
                allowed,
                ok == Some(true),
                format!("Fairfax Table 3, {w}' influence width, span up to {s}'"),
            ));
        } else {
            let allowed = irc_beam_span(deck.species, deck.beam, deck.beam_plies, beam.effective);
            out.push(check(
                &format!("Beam span, {}", beam.name),
                format!("{}", beam.span),
                allowed.map_or("not in table".to_string(), |(a, _)| a.to_string()),
                allowed.is_some_and(|(a, _)| beam.span <= a),
                format!(
                    "IRC Table R507.5, {} {name}, effective joist span {} (up to {})",
                    deck.species.name(),
                    beam.effective,
                    allowed.map_or("over 18'".to_string(), |(_, col)| col.to_string())
                ),
            ));
        }
        let quarter = beam.span.mul_div(1, 4);
        out.push(check(
            &format!("Beam past its end posts, {}", beam.name),
            format!("{}", beam.end_overhang),
            format!("{quarter} (1/4 of the span)"),
            beam.end_overhang <= quarter,
            if fairfax {
                "Fairfax Section 4".to_string()
            } else {
                "IRC R507.5".to_string()
            },
        ));
    }

    // Posts and footings.
    let post_width = match deck.support {
        DeckSupport::Piers => deck.beam.actual_thickness() * i64::from(deck.beam_plies),
        _ => deck.post.actual_width(),
    };
    if let Some(h) = layout.post_height {
        if fairfax {
            let cap = layout
                .beams
                .iter()
                .filter_map(|b| fairfax_cell(b.influence, b.span))
                .filter_map(|(r, c, ..)| FAIRFAX_FOOTINGS[r][c])
                .map(|(_, ft)| Length::feet(i64::from(ft)))
                .min()
                .unwrap_or(Length::feet(14));
            let six = Nominal::new(6, 6);
            let size_ok =
                deck.post == six || (deck.post == Nominal::new(4, 4) && h <= FAIRFAX_4X4_POST);
            out.push(check(
                "Post size",
                format!("{} posts {h} tall", deck.post),
                format!("6x6; 4x4 only up to {FAIRFAX_4X4_POST}"),
                size_ok,
                "Fairfax Section 5".to_string(),
            ));
            out.push(check(
                "Post height",
                format!("{h}"),
                format!("{cap}"),
                h <= cap,
                "Fairfax Table 4".to_string(),
            ));
            out.push(check(
                "Lateral bracing",
                format!("posts {h} tall"),
                if h > FAIRFAX_BRACE_ABOVE {
                    "knee braces or other lateral support per Table 8 (methods 2 and 3); Sticks doesn't draw them yet".to_string()
                } else {
                    format!("none needed for posts {FAIRFAX_BRACE_ABOVE} or shorter")
                },
                h <= FAIRFAX_BRACE_ABOVE,
                "Fairfax Table 8".to_string(),
            ));
        } else {
            let cap = irc_post_height(deck.post);
            out.push(check(
                "Post height",
                format!("{} posts {h} tall", deck.post),
                cap.map_or("not in table".to_string(), |c| c.to_string()),
                cap.is_some_and(|c| h <= c),
                "IRC Table R507.4".to_string(),
            ));
            if h > Length::feet(2) {
                out.push(check(
                    "Lateral bracing",
                    format!("posts {h} tall"),
                    "knee braces on a freestanding deck with posts over 2'; Sticks doesn't draw them yet".to_string(),
                    false,
                    "AWC DCA 6 (freestanding decks)".to_string(),
                ));
            }
        }
    }
    if fairfax && deck.support == DeckSupport::Piers {
        out.push(check(
            "Beam support",
            "beams straight on concrete piers".to_string(),
            "posts on footings, with post bases and post caps".to_string(),
            false,
            "Fairfax Section 5 (ask the county before setting beams on piers)".to_string(),
        ));
    }
    match deck.support {
        DeckSupport::Blocks => out.push(check(
            "Footings",
            "precast deck blocks on the ground".to_string(),
            "concrete footings below the frost line".to_string(),
            false,
            if fairfax {
                "Fairfax Section 5 (deck blocks aren't in the details)".to_string()
            } else {
                "IRC R507.3 (blocks only with joists bearing straight on them, under 200 sq ft and 20\" high)".to_string()
            },
        )),
        DeckSupport::Piers | DeckSupport::Posts => {
            let biggest = layout
                .beams
                .iter()
                .flat_map(|b| footings(deck, b, post_width))
                .max_by_key(|f| f.side);
            if let Some(f) = biggest {
                let size = match f.kind {
                    Some(k) => format!(
                        "type {k}: {:#} square or {:#} round, {:#} thick",
                        f.side, f.diameter, f.thickness
                    ),
                    None => format!(
                        "{:#} square or {:#} round, {:#} thick",
                        f.side, f.diameter, f.thickness
                    ),
                };
                out.push(check(
                    "Footing size",
                    format!("{} sq ft of deck, {} lb", f.area_sqft, f.load_lb),
                    size,
                    f.side > Length::ZERO,
                    if fairfax {
                        "Fairfax Tables 4 and 5".to_string()
                    } else {
                        format!(
                            "IRC R507.3.1 basis: {} psf live + {DEAD_PSF} psf dead on {} psf soil",
                            deck.live_psf, deck.soil_psf
                        )
                    },
                ));
                out.push(check(
                    "Footing depth",
                    format!("bottom {:#} below grade", f.depth),
                    if fairfax {
                        "24\" below grade, on solid ground, inspected before the pour".to_string()
                    } else {
                        match deck.frost_depth {
                            Some(d) => format!("12\" below undisturbed ground and below the {d:#} frost line"),
                            None => "12\" below undisturbed ground; below the frost line where your building department requires it".to_string(),
                        }
                    },
                    true,
                    if fairfax {
                        "Fairfax Section 5, Figure 17".to_string()
                    } else {
                        "IRC R507.3.2".to_string()
                    },
                ));
            }
        }
    }

    // Decking.
    let allowed = decking_spacing(deck.decking, deck.decking_material);
    out.push(check(
        "Decking span",
        format!(
            "{} {} over joists {oc:#} o.c.",
            deck.decking,
            match deck.decking_material {
                DeckingMaterial::Wood => "wood",
                DeckingMaterial::Composite => "composite",
            }
        ),
        allowed.map_or("what the board's maker lists".to_string(), |a| {
            format!("{a:#} o.c. or less")
        }),
        allowed.is_none_or(|a| oc <= a),
        if fairfax {
            "Fairfax Table 1".to_string()
        } else {
            "IRC Table R507.7".to_string()
        },
    ));

    // Guards.
    let need_guard = deck.height > GUARD_ABOVE;
    out.push(check(
        "Guards",
        format!("decking {} above grade", deck.height),
        if need_guard {
            "36\" guards on open sides, openings under 4\"; Sticks doesn't draw guards yet"
                .to_string()
        } else {
            format!("none needed at {GUARD_ABOVE} or less")
        },
        !need_guard,
        if fairfax {
            "Fairfax Section 9".to_string()
        } else {
            "IRC R312.1".to_string()
        },
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(s: &str) -> Length {
        s.parse().unwrap()
    }

    #[test]
    fn joist_spans_read_the_next_spacing_up() {
        let sp = Species::SouthernPine;
        let j = Nominal::new(2, 8);
        assert_eq!(joist_span(sp, j, l("16")), Some(l("11' 10")));
        assert_eq!(joist_span(sp, j, l("19-3/16")), Some(l("9' 8")));
        assert_eq!(joist_span(sp, j, l("30")), None);
        assert_eq!(joist_span(sp, Nominal::new(2, 4), l("16")), None);
        let irc = DeckCode::Irc2021;
        // A quarter of a 6' back span is less than the table's 2'.
        assert_eq!(joist_overhang(irc, sp, j, l("16"), l("6'")), Some(l("18")));
        assert_eq!(joist_overhang(irc, sp, j, l("16"), l("10'")), Some(l("2'")));
    }

    #[test]
    fn fairfax_overhangs_take_the_lesser_column() {
        let ff = DeckCode::Fairfax2021;
        let sp = Species::SouthernPine;
        let j = Nominal::new(2, 8);
        // 11' sits between the 10' (2'-6") and 12' (2'-3") columns.
        assert_eq!(
            joist_overhang(ff, sp, j, l("16"), l("11'")),
            Some(l("2' 3"))
        );
        // 2x8s can't overhang past a 14' span.
        assert_eq!(
            joist_overhang(ff, sp, j, l("16"), l("13'")),
            Some(Length::ZERO)
        );
    }

    #[test]
    fn irc_beam_spans_read_the_next_joist_span_up() {
        let sp = Species::SouthernPine;
        let b = Nominal::new(2, 8);
        assert_eq!(
            irc_beam_span(sp, b, 2, l("12'")),
            Some((l("6' 2"), l("12'")))
        );
        assert_eq!(
            irc_beam_span(sp, b, 2, l("12' 1")),
            Some((l("5' 9"), l("14'")))
        );
        assert_eq!(irc_beam_span(sp, b, 2, l("19'")), None);
        // Southern pine solid 4x beams aren't in the table Sticks carries.
        assert_eq!(irc_beam_span(sp, Nominal::new(4, 6), 1, l("8'")), None);
        assert_eq!(
            irc_beam_span(Species::DouglasFir, Nominal::new(4, 6), 1, l("8'")),
            Some((l("5' 6"), l("8'")))
        );
    }

    #[test]
    fn fairfax_beams_and_footings() {
        let b = Nominal::new(2, 10);
        // 6' of joists on a beam spanning 8': (2) 2x8 or (3) 2x6 at least.
        assert_eq!(fairfax_beam_ok(b, 2, l("6'"), l("8'")), Some(true));
        assert_eq!(
            fairfax_beam_ok(Nominal::new(2, 6), 2, l("6'"), l("8'")),
            Some(false)
        );
        assert_eq!(
            fairfax_beam_ok(Nominal::new(2, 6), 3, l("6'"), l("8'")),
            Some(true)
        );
        // 6' 1" rounds up to the 7' row.
        assert_eq!(
            fairfax_beam_ok(Nominal::new(2, 8), 2, l("6' 1"), l("8'")),
            Some(false)
        );
        // Shaded: too wide for that span.
        assert_eq!(fairfax_beam_ok(b, 2, l("10'"), l("12'")), None);
        let deck = Deck {
            code: DeckCode::Fairfax2021,
            beam: b,
            ..Deck::default()
        };
        assert_eq!(longest_beam_span(&deck, l("6'"), l("12'")), Some(l("10'")));
    }

    #[test]
    fn a_cantilever_loads_the_beam_more() {
        assert_eq!(effective_joist_span(l("8'"), Length::ZERO), l("8'"));
        // (8 + 2)^2 / 8 = 12.5'.
        assert_eq!(effective_joist_span(l("8'"), l("2'")), l("12' 6"));
    }

    #[test]
    fn irc_footings_grow_with_their_load() {
        // 30 sq ft at 50 psf is 1,500 lb, about a square foot on 1,500 psf soil.
        let f = irc_footing(30 * 144, 40, 1500, l("3-1/2"));
        assert_eq!(f.load_lb, 1500);
        assert_eq!(f.side, l("13"));
        assert_eq!(f.diameter, l("15"));
        assert_eq!(f.thickness, l("6"));
        let small = irc_footing(4 * 144, 40, 1500, l("3-1/2"));
        assert_eq!(small.side, l("12"));
        let big = irc_footing(80 * 144, 40, 1500, l("5-1/2"));
        assert!(big.side > l("20") && big.thickness > l("6"), "{big:?}");
    }

    #[test]
    fn decking_spans() {
        let wood = DeckingMaterial::Wood;
        assert_eq!(
            decking_spacing("5/4x6".parse().unwrap(), wood),
            Some(l("16"))
        );
        assert_eq!(decking_spacing(Nominal::new(2, 6), wood), Some(l("24")));
        assert_eq!(
            decking_spacing(Nominal::new(2, 6), DeckingMaterial::Composite),
            None
        );
    }
}
