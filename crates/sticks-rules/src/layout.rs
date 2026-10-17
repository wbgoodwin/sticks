//! Helpers shared by the wall, floor and roof generators: on-center layout,
//! the geometry types members are placed with, and splitting long runs.

use serde::Serialize;
use sticks_core::{LayoutStart, Length, Member, MemberKind, Nominal};

/// An axis-aligned rectangle: a wall elevation, or a floor or roof plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Rect {
    pub x: Length,
    pub y: Length,
    pub w: Length,
    pub h: Length,
}

impl Rect {
    pub fn right(&self) -> Length {
        self.x + self.w
    }

    pub fn top(&self) -> Length {
        self.y + self.h
    }
}

/// A member and where it sits in its drawing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Placed {
    pub member: Member,
    /// The member's bounding box.
    pub rect: Rect,
    /// Corners in order, for members that aren't rectangles in the drawing
    /// (sloped plates, studs with an angled top). Empty when `rect` is the shape.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub outline: Vec<(Length, Length)>,
}

/// A measurement for build day: where a member's near edge lands, measured
/// from the end the layout starts at.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LayoutMark {
    pub label: String,
    pub kind: MemberKind,
    pub at: Length,
}

/// A butt joint where a run longer than any stock board is split.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Splice {
    pub kind: MemberKind,
    /// Distance from the left end (or the front, for beams) to the joint.
    pub at: Length,
}

pub fn placed(
    label: String,
    kind: MemberKind,
    size: Nominal,
    rect: Rect,
    vertical: bool,
) -> Placed {
    Placed {
        member: Member {
            label,
            kind,
            size,
            length: if vertical { rect.h } else { rect.w },
            cuts: Vec::new(),
        },
        rect,
        outline: Vec::new(),
    }
}

/// Near edges of on-center members of thickness `t` along a run of `len`,
/// leaving out the two flush end members. Centers fall on multiples of
/// `on_center` from the layout end, so sheet edges land on member centers.
pub fn on_center(len: Length, on_center: Length, t: Length, from: LayoutStart) -> Vec<Length> {
    let mut out = Vec::new();
    let mut k = 1;
    loop {
        let center = on_center * k;
        let x = match from {
            LayoutStart::Left => center - half(t),
            LayoutStart::Right => len - center - half(t),
        };
        if x < t || x + t > len - t {
            break;
        }
        out.push(x);
        k += 1;
    }
    out.sort();
    out
}

/// Joints that split the run from `a` to `b` into pieces that each `fit`
/// (given a piece's level span). Joints land only on `candidates` and stay at
/// least `offset` away from every joint in `avoid`. Each piece is made as long
/// as it can be, working from `a`. `None` when no set of joints works.
pub fn split_run(
    a: Length,
    b: Length,
    fits: impl Fn(Length) -> bool,
    candidates: &[Length],
    avoid: &[Length],
    offset: Length,
) -> Option<Vec<Length>> {
    let mut joints = Vec::new();
    let mut start = a;
    while !fits(b - start) {
        let next = candidates
            .iter()
            .copied()
            .filter(|&c| c > start && c < b && fits(c - start))
            .filter(|&c| avoid.iter().all(|&j| (c - j).abs() >= offset))
            .max()?;
        joints.push(next);
        start = next;
    }
    Some(joints)
}

/// `[a, b]` cut at each joint, as `(start, end)` pieces.
pub fn pieces(a: Length, b: Length, joints: &[Length]) -> Vec<(Length, Length)> {
    let mut out = Vec::new();
    let mut start = a;
    for &j in joints {
        out.push((start, j));
        start = j;
    }
    out.push((start, b));
    out
}

pub fn half(l: Length) -> Length {
    Length::from_sixteenths(l.sixteenths() / 2)
}

pub fn overlaps(a: (Length, Length), b: (Length, Length)) -> bool {
    a.0 < b.1 && b.0 < a.1
}

/// `n` pieces of length `piece` spread evenly from `start` over `span`.
pub fn spread(start: Length, span: Length, piece: Length, n: i64) -> Vec<Length> {
    if n == 1 {
        return vec![start + half(span - piece)];
    }
    let step = (span - piece).sixteenths() / (n - 1);
    (0..n)
        .map(|i| start + Length::from_sixteenths(step * i))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(s: &str) -> Length {
        s.parse().unwrap()
    }

    #[test]
    fn splits_at_the_farthest_candidate_that_fits() {
        let studs: Vec<_> = (1..14).map(|k| Length::inches(16 * k)).collect();
        let fits = |span: Length| span <= l("16'");
        let joints = split_run(Length::ZERO, l("18'"), fits, &studs, &[], Length::ZERO).unwrap();
        assert_eq!(joints, [l("192")]);
        // Offset from the first plate's joint by at least 24".
        let cap = split_run(Length::ZERO, l("18'"), fits, &studs, &joints, l("24")).unwrap();
        assert_eq!(cap, [l("160")]);
        assert_eq!(
            pieces(Length::ZERO, l("18'"), &cap),
            [(Length::ZERO, l("160")), (l("160"), l("216"))]
        );
        assert!(split_run(Length::ZERO, l("18'"), fits, &[l("12")], &[], Length::ZERO).is_none());
    }
}
