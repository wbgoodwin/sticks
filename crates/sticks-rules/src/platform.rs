//! Building blocks shared by every framed platform (the shed floor, the
//! porch deck and a deck on its own): joists on center, rims spliced over a
//! joist, built-up beams spliced over their supports, a line of piers along
//! a beam, and decking boards across the joists.
//!
//! All of them work in plan: `x` along the rims and beams, `y` along the
//! joists.

use sticks_core::{longest_board, LayoutStart, Length, MemberKind, Nominal, Rules, Stock};

use crate::layout::{
    half, on_center, pieces, placed, ply_joints, split_run, LayoutMark, Placed, Rect, Splice,
};

/// Near edges of the joists across `width`: one flush with each end and the
/// rest on layout, and their centers.
pub fn joist_layout(
    width: Length,
    oc: Length,
    t: Length,
    from: LayoutStart,
) -> (Vec<Length>, Vec<Length>) {
    let mut x = vec![Length::ZERO];
    x.extend(on_center(width, oc, t, from));
    x.push(width - t);
    let centers = x.iter().map(|&x| x + half(t)).collect();
    (x, centers)
}

/// Layout marks for joists at `joist_x`, measured from the layout end.
pub fn joist_marks(
    prefix: &str,
    joist_x: &[Length],
    t: Length,
    width: Length,
    from: LayoutStart,
) -> Vec<LayoutMark> {
    let mut marks: Vec<LayoutMark> = joist_x
        .iter()
        .enumerate()
        .map(|(i, &x)| LayoutMark {
            label: format!("{prefix}{:02}", i + 1),
            kind: MemberKind::Joist,
            at: match from {
                LayoutStart::Left => x,
                LayoutStart::Right => width - x - t,
            },
        })
        .collect();
    marks.sort_by_key(|m| m.at);
    marks
}

/// Joints that split a run of `board` across `width` over the inner joist
/// `centers`, or why it can't be split.
pub fn joints_over_joists(
    what: &str,
    board: Nominal,
    width: Length,
    centers: &[Length],
    stock: &[Stock],
    rules: &Rules,
) -> Result<Vec<Length>, String> {
    let longest = longest_board(stock, board, rules.end_trim);
    let inner = &centers[1..centers.len() - 1];
    split_run(Length::ZERO, width, |s| s <= longest, inner, &[], Length::ZERO).ok_or_else(|| {
        format!(
            "the {what} is longer than the longest {board} stock ({longest}) and no joist lands where it could be spliced; add longer stock"
        )
    })
}

/// A board run across `width` at `y`, cut at `joints`, labeled
/// `{prefix}1`, `{prefix}2`... from `first`.
#[allow(clippy::too_many_arguments)]
pub fn run_pieces(
    prefix: &str,
    first: usize,
    kind: MemberKind,
    board: Nominal,
    width: Length,
    y: Length,
    depth: Length,
    joints: &[Length],
) -> Vec<Placed> {
    pieces(Length::ZERO, width, joints)
        .into_iter()
        .enumerate()
        .map(|(n, (a, b))| {
            placed(
                format!("{prefix}{}", first + n),
                kind,
                board,
                Rect {
                    x: a,
                    y,
                    w: b - a,
                    h: depth,
                },
                false,
            )
        })
        .collect()
}

/// Supports along a beam of `width`, evenly spaced from one `inset` end to
/// the other, no more than `spacing` apart.
pub fn support_line(width: Length, inset: Length, spacing: Length) -> Vec<Length> {
    let span = width - inset * 2;
    let gaps = ((span.sixteenths() + spacing.sixteenths() - 1) / spacing.sixteenths()).max(1);
    (0..=gaps).map(|k| inset + span.mul_div(k, gaps)).collect()
}

/// One built-up beam line across `width` with its outer face at `y`, each
/// ply cut at its own `joints`. Labels run `{label}a-1`, `{label}a-2`...,
/// leaving out the ply letter for a single ply and the number for an uncut
/// ply.
#[allow(clippy::too_many_arguments)]
pub fn beam_line(
    label: &str,
    kind: MemberKind,
    board: Nominal,
    width: Length,
    y: Length,
    joints: &[Vec<Length>],
) -> (Vec<Placed>, Vec<Splice>) {
    let ply = board.actual_thickness();
    let mut members = Vec::new();
    let mut splices = Vec::new();
    for (p, ply_joints) in joints.iter().enumerate() {
        let runs = pieces(Length::ZERO, width, ply_joints);
        let many = runs.len() > 1;
        for (k, (a, b)) in runs.into_iter().enumerate() {
            let mut name = label.to_string();
            if joints.len() > 1 {
                name.push((b'a' + p as u8) as char);
            }
            if many {
                name.push_str(&format!("-{}", k + 1));
            }
            members.push(placed(
                name,
                kind,
                board,
                Rect {
                    x: a,
                    y: y + ply * p as i64,
                    w: b - a,
                    h: ply,
                },
                false,
            ));
        }
        splices.extend(ply_joints.iter().map(|&at| Splice { kind, at }));
    }
    (members, splices)
}

/// Joints for each ply of a beam across `width` over `supports`, or why it
/// can't be spliced. The flag is set when two plies share their joints.
pub fn beam_joints(
    board: Nominal,
    plies: u8,
    width: Length,
    supports: &[Length],
    over: &str,
    stock: &[Stock],
    rules: &Rules,
) -> Result<(Vec<Vec<Length>>, bool), String> {
    let longest = longest_board(stock, board, rules.end_trim);
    ply_joints(width, plies, supports, longest).ok_or_else(|| {
        format!(
            "the beam is longer than the longest {board} stock ({longest}) and no {over} lands where it could be spliced; add {over}s or longer stock"
        )
    })
}

/// Decking boards across the joists in rows from `y0` toward `y1`: a full
/// board at `y0`, any rip at `y1`, end joints on joists and staggered row
/// to row by at least one joist space.
pub struct Decking<'a> {
    /// Label prefix, like `porch-DK`.
    pub prefix: &'a str,
    pub width: Length,
    pub y0: Length,
    pub y1: Length,
    /// Joist centers the boards can end on, both end joists included.
    pub centers: &'a [Length],
    pub board: Nominal,
    pub gap: Length,
    pub oc: Length,
    /// Where the last row is, for the ripped-board note, like "against the shed".
    pub rip_at: &'a str,
}

pub struct LaidDecking {
    pub members: Vec<Placed>,
    pub rows: usize,
    pub warnings: Vec<String>,
}

impl Decking<'_> {
    pub fn lay(&self, stock: &[Stock], rules: &Rules) -> Result<LaidDecking, String> {
        let board_w = self.board.actual_width();
        let longest = longest_board(stock, self.board, rules.end_trim);
        let inner = &self.centers[1..self.centers.len() - 1];
        let down = self.y1 < self.y0;
        let mut members = Vec::new();
        let mut warnings = Vec::new();
        let mut done = Length::ZERO;
        let total = (self.y1 - self.y0).abs();
        let mut rows = 0;
        let mut last: Vec<Length> = Vec::new();
        while done < total {
            let w = board_w.min(total - done);
            let joints = split_run(Length::ZERO, self.width, |s| s <= longest, inner, &last, self.oc)
                .or_else(|| {
                    split_run(Length::ZERO, self.width, |s| s <= longest, inner, &[], Length::ZERO)
                })
                .ok_or_else(|| {
                    format!(
                        "decking is longer than the longest {} stock ({longest}) and no joist lands where it could be spliced; add longer stock",
                        self.board
                    )
                })?;
            rows += 1;
            let y = if down {
                self.y0 - done - w
            } else {
                self.y0 + done
            };
            let runs = pieces(Length::ZERO, self.width, &joints);
            let many = runs.len() > 1;
            for (k, (a, b)) in runs.into_iter().enumerate() {
                let mut label = format!("{}{rows:02}", self.prefix);
                if many {
                    label.push((b'a' + k as u8) as char);
                }
                members.push(placed(
                    label,
                    MemberKind::Decking,
                    self.board,
                    Rect {
                        x: a,
                        y,
                        w: b - a,
                        h: w,
                    },
                    false,
                ));
            }
            if w < board_w {
                warnings.push(format!(
                    "the last row of decking, {}, is ripped to {w:#}",
                    self.rip_at
                ));
            }
            last = joints;
            done += w + self.gap;
        }
        Ok(LaidDecking {
            members,
            rows,
            warnings,
        })
    }
}
