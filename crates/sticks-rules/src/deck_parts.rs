//! The parts of a deck beyond its platform: guards along the open sides,
//! stairs down to the ground, the ledger's bolts into the house, and the
//! concrete it all stands on.
//!
//! A side is worked in its own coordinates, `a` along the side from its
//! start (the left end of the front or back, the front end of the left or
//! right) and `r` out from the deck's edge, then turned into plan.

use serde::Serialize;
use sticks_core::{
    longest_board, Deck, DeckSide, DeckStairs, DeckingMaterial, LedgerFasteners, Length,
    MemberKind, Nominal, Rules, Stock,
};

use crate::code::decking_spacing;
use crate::layout::{half, pieces, placed, split_run, Placed, Rect};

/// Guards are this tall over the decking (IRC R312.1.2).
pub const GUARD_HEIGHT: Length = Length::inches(36);
/// Guard posts no farther apart than this, center to center (AWC DCA 6).
pub const GUARD_POST_SPACING: Length = Length::inches(72);
/// Room between balusters, and under the bottom rail, kept under the 4"
/// ball the code tests with.
pub const MAX_GAP: Length = Length::from_sixteenths(60); // 3-3/4"
/// Most a riser may be (IRC R311.7.5.1).
pub const MAX_RISER: Length = Length::from_sixteenths(124); // 7-3/4"
/// Least a tread may be, nosing to nosing (IRC R311.7.5.2).
pub const MIN_TREAD: Length = Length::inches(10);
/// Least stair width (IRC R311.7.1).
pub const MIN_STAIR_WIDTH: Length = Length::inches(36);
/// How far a tread's nosing reaches past the riser below.
pub const NOSING: Length = Length::from_sixteenths(14); // 7/8"
/// Longest horizontal run a cut stringer may span (AWC DCA 6).
pub const STRINGER_SPAN: Length = Length::inches(72);
/// Most room between cut stringers, center to center (AWC DCA 6).
pub const STRINGER_SPACING: Length = Length::inches(18);
/// Least depth left in a cut stringer below its notches (AWC DCA 6).
pub const MIN_THROAT: Length = Length::inches(5);
/// Handrail height over the nosings, within the code's 34"-38".
pub const HANDRAIL_HEIGHT: Length = Length::inches(34);
/// Landing pad at the foot of a stair: depth out from the last riser, and
/// thickness.
pub const PAD_DEPTH: Length = Length::inches(36);
pub const PAD_THICKNESS: Length = Length::inches(4);

const POST: Nominal = Nominal::new(4, 4);
const RAIL: Nominal = Nominal::new(2, 4);
const BALUSTER: Nominal = Nominal::new(2, 2);
const HANDRAIL: Nominal = Nominal::new(2, 2);
const RISER_BOARD: Nominal = Nominal::new(1, 8);

/// How a piece stands in 3D. A level piece fills its plan rectangle from
/// `lo` to `hi`; a sloped one (a stringer, a stair rail) is `profile`, a
/// side view in (distance along the plan axis, height), pushed across its
/// plan rectangle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Rise {
    pub label: String,
    pub lo: Length,
    pub hi: Length,
    /// Side view corners, empty for a level piece.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub profile: Vec<(Length, Length)>,
    /// Whether the profile's distance runs along plan `x` (else plan `y`).
    pub along_x: bool,
}

impl Rise {
    fn level(label: &str, lo: Length, hi: Length) -> Rise {
        Rise {
            label: label.to_string(),
            lo,
            hi,
            profile: Vec::new(),
            along_x: false,
        }
    }
}

/// A plan rectangle from a side's own `a` (along) and `r` (out) ranges.
pub fn side_rect(
    side: DeckSide,
    width: Length,
    depth: Length,
    (a0, a1): (Length, Length),
    (r0, r1): (Length, Length),
) -> Rect {
    match side {
        DeckSide::Front => Rect {
            x: a0,
            y: -r1,
            w: a1 - a0,
            h: r1 - r0,
        },
        DeckSide::Back => Rect {
            x: a0,
            y: depth + r0,
            w: a1 - a0,
            h: r1 - r0,
        },
        DeckSide::Left => Rect {
            x: -r1,
            y: a0,
            w: r1 - r0,
            h: a1 - a0,
        },
        DeckSide::Right => Rect {
            x: width + r0,
            y: a0,
            w: r1 - r0,
            h: a1 - a0,
        },
    }
}

/// Plan coordinate, along the axis `r` runs on, of a point `r` out.
fn out_coord(side: DeckSide, width: Length, depth: Length, r: Length) -> Length {
    match side {
        DeckSide::Front | DeckSide::Left => -r,
        DeckSide::Back => depth + r,
        DeckSide::Right => width + r,
    }
}

/// Whether `a` runs along plan `x`.
fn a_along_x(side: DeckSide) -> bool {
    matches!(side, DeckSide::Front | DeckSide::Back)
}

pub fn side_length(side: DeckSide, width: Length, depth: Length) -> Length {
    if a_along_x(side) {
        width
    } else {
        depth
    }
}

/// Evenly spaced positions for `count` pieces `size` wide filling the
/// room from `a` to `b` with equal gaps, by their near edges.
fn spread(a: Length, b: Length, size: Length, count: i64) -> Vec<Length> {
    let room = b - a - size * count;
    (0..count)
        .map(|k| a + room.mul_div(k + 1, count + 1) + size * k)
        .collect()
}

/// Balusters to fill a clear gap so no space is over [`MAX_GAP`].
fn baluster_count(clear: Length) -> i64 {
    let (w, g) = (BALUSTER.actual_width().sixteenths(), MAX_GAP.sixteenths());
    let c = clear.sixteenths();
    if c <= g {
        0
    } else {
        // (c - k w) / (k + 1) <= g  =>  k >= (c - g) / (w + g)
        (c - g + w + g - 1) / (w + g)
    }
}

/// Guards along a deck's open sides.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FramedGuards {
    /// The sides that have them.
    pub sides: Vec<DeckSide>,
    /// Widest center-to-center between posts.
    pub post_spacing: Length,
    /// Widest space between balusters or beside a post.
    pub gap: Length,
    pub height: Length,
    pub posts: usize,
    pub members: Vec<Placed>,
    pub rises: Vec<Rise>,
}

/// Inputs shared by guards and stairs.
pub struct Edge<'a> {
    pub deck: &'a Deck,
    pub width: Length,
    pub depth: Length,
    /// Top of the decking.
    pub top: Length,
    /// Bottom of the rims and joists, where guard posts are bolted down to.
    pub joist_bottom: Length,
    pub stock: &'a [Stock],
    pub rules: &'a Rules,
}

impl Edge<'_> {
    fn rect(&self, side: DeckSide, a: (Length, Length), r: (Length, Length)) -> Rect {
        side_rect(side, self.width, self.depth, a, r)
    }

    fn longest(&self, size: Nominal) -> Length {
        longest_board(self.stock, size, self.rules.end_trim)
    }
}

/// Posts outside the rim of each guarded `side`, rails on their inner
/// faces, balusters between, leaving a gap at each stair.
pub fn frame_guards(e: &Edge, sides: &[DeckSide]) -> Result<FramedGuards, String> {
    let pw = POST.actual_width();
    let rt = RAIL.actual_thickness();
    let top = e.top;
    let height = GUARD_HEIGHT;
    let mut members = Vec::new();
    let mut rises = Vec::new();
    let mut spacing = Length::ZERO;
    let mut widest_gap = Length::ZERO;
    let mut posts = 0;
    let (mut rails, mut balusters) = (0, 0);
    let post_len = top + height - e.joist_bottom;
    let post_rise = (e.joist_bottom, top + height);
    let rail_z = [
        (top + height - RAIL.actual_width(), top + height),
        (top + MAX_GAP, top + MAX_GAP + RAIL.actual_width()),
    ];
    let baluster_z = (top + MAX_GAP, top + height);
    for &side in sides {
        let len = side_length(side, e.width, e.depth);
        // Stair openings on this side, by their `a` range.
        let mut gaps: Vec<(Length, Length)> = e
            .deck
            .stairs
            .iter()
            .filter(|s| s.side == side)
            .map(|s| (s.at, s.at + s.width))
            .collect();
        gaps.sort();
        let mut segments = Vec::new();
        let mut start = Length::ZERO;
        for (a, b) in gaps {
            segments.push((start, a.max(start)));
            start = start.max(b);
        }
        segments.push((start, len));
        // Rails on the left and right stop short of the front and back
        // rails they'd run into.
        let trim = |end: DeckSide| {
            if !a_along_x(side) && sides.contains(&end) {
                rt
            } else {
                Length::ZERO
            }
        };
        for (s, t) in segments {
            if t - s < pw {
                continue;
            }
            let bays = ((t - s - pw).sixteenths() + GUARD_POST_SPACING.sixteenths() - 1)
                / GUARD_POST_SPACING.sixteenths();
            let at: Vec<Length> = (0..=bays.max(1))
                .map(|k| s + (t - s - pw).mul_div(k, bays.max(1)))
                .collect();
            let at: Vec<Length> = if t - s < pw * 2 { vec![s] } else { at };
            for w in at.windows(2) {
                spacing = spacing.max(w[1] - w[0]);
                let clear = w[1] - w[0] - pw;
                let k = baluster_count(clear);
                let bw = BALUSTER.actual_width();
                let gap = (clear - bw * k).mul_div(1, k + 1);
                widest_gap = widest_gap.max(gap);
                for x in spread(w[0] + pw, w[1], bw, k) {
                    balusters += 1;
                    let label = format!("deck-GB{balusters:03}");
                    let mut p = placed(
                        label.clone(),
                        MemberKind::Baluster,
                        BALUSTER,
                        e.rect(side, (x, x + bw), (Length::ZERO, bw)),
                        false,
                    );
                    p.member.length = baluster_z.1 - baluster_z.0;
                    members.push(p);
                    rises.push(Rise::level(&label, baluster_z.0, baluster_z.1));
                }
            }
            for &a in &at {
                posts += 1;
                let label = format!("deck-GP{posts:02}");
                let mut p = placed(
                    label.clone(),
                    MemberKind::GuardPost,
                    POST,
                    e.rect(side, (a, a + pw), (Length::ZERO, pw)),
                    false,
                );
                p.member.length = post_len;
                members.push(p);
                rises.push(Rise::level(&label, post_rise.0, post_rise.1));
            }
            // Top and bottom rails, spliced over posts when stock runs short.
            let (ra, rb) = (
                if s == Length::ZERO {
                    s + trim(DeckSide::Front)
                } else {
                    s
                },
                if t == len {
                    t - trim(DeckSide::Back)
                } else {
                    t
                },
            );
            let centers: Vec<Length> = at.iter().map(|&a| a + half(pw)).collect();
            let longest = e.longest(RAIL);
            let joints = split_run(ra, rb, |l| l <= longest, &centers, &[], Length::ZERO)
                .ok_or_else(|| {
                    format!(
                        "a guard rail on the {} is longer than the longest {RAIL} stock ({longest}) between posts; add longer stock",
                        side.name()
                    )
                })?;
            for (lo, hi) in rail_z {
                for (a, b) in pieces(ra, rb, &joints) {
                    rails += 1;
                    let label = format!("deck-GR{rails:02}");
                    let mut p = placed(
                        label.clone(),
                        MemberKind::Rail,
                        RAIL,
                        e.rect(side, (a, b), (-rt, Length::ZERO)),
                        false,
                    );
                    p.member.length = b - a;
                    members.push(p);
                    rises.push(Rise::level(&label, lo, hi));
                }
            }
        }
    }
    Ok(FramedGuards {
        sides: sides.to_vec(),
        post_spacing: spacing,
        gap: widest_gap,
        height,
        posts,
        members,
        rises,
    })
}

/// A flight of stairs, worked out.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FramedStairs {
    pub side: DeckSide,
    pub at: Length,
    pub width: Length,
    /// Decking top to the pad.
    pub rise: Length,
    pub risers: i64,
    /// The tallest riser.
    pub riser: Length,
    /// Nosing to nosing.
    pub run: Length,
    /// From the deck's edge to the face of the last riser.
    pub total_run: Length,
    pub tread: Nominal,
    pub tread_depth: Length,
    pub stringer: Nominal,
    pub stringers: usize,
    /// Widest center to center between stringers.
    pub stringer_spacing: Length,
    /// Least depth left in a stringer under its notches.
    pub throat: Length,
    pub handrail: bool,
    pub guards: bool,
    /// Riser boards close the openings between treads.
    pub closed_risers: bool,
    /// The concrete landing pad, in plan.
    pub pad: Rect,
    pub members: Vec<Placed>,
    pub rises: Vec<Rise>,
    /// Plan rectangles of the posts at the top, so a guard post already
    /// there isn't doubled.
    pub top_posts: Vec<Rect>,
}

/// Stairs from the deck's `side` down to a concrete pad at grade.
pub fn frame_stairs(
    e: &Edge,
    n: usize,
    s: &DeckStairs,
    guard_posts: &[Rect],
) -> Result<FramedStairs, String> {
    let side = s.side;
    let len = side_length(side, e.width, e.depth);
    let name = format!("the {} stairs", side.name());
    if s.width <= Length::ZERO || s.at < Length::ZERO || s.at + s.width > len {
        return Err(format!(
            "{name} {} wide at {} don't fit along the {len} side",
            s.width, s.at
        ));
    }
    let h = e.top;
    let risers = (h.sixteenths() + MAX_RISER.sixteenths() - 1) / MAX_RISER.sixteenths();
    if risers < 2 {
        return Err(format!(
            "the decking is only {h} up; {name} would be a single step"
        ));
    }
    let z = |i: i64| h - h.mul_div(i, risers);
    let riser = (0..risers)
        .map(|i| z(i) - z(i + 1))
        .max()
        .unwrap_or_default();
    let tw = s.tread.actual_width();
    let tt = s.tread.actual_thickness();
    let gap = e.deck.gap;
    let tread_depth = tw * 2 + gap;
    let run = tread_depth - NOSING;
    let treads = risers - 1;
    let total_run = run * treads;
    let closed = h > crate::code::GUARD_ABOVE;
    let back = if closed {
        RISER_BOARD.actual_thickness()
    } else {
        Length::ZERO
    };

    let mut members = Vec::new();
    let mut rises = Vec::new();
    let pre = format!("deck-S{n}");
    let along_x = !a_along_x(side);
    let c = |r: Length| out_coord(side, e.width, e.depth, r);

    // Stringers, cut from the slope's nose line down by the board's depth.
    let st = s.stringer.actual_thickness();
    let sd = s.stringer.actual_width();
    let slope_len = ((run.as_inches_f64()).powi(2) + (riser.as_inches_f64()).powi(2)).sqrt();
    let drop = Length::from_inches_f64(sd.as_inches_f64() * slope_len / run.as_inches_f64());
    let rise_at = |r: Length| h - tt - riser.mul_div(r.sixteenths(), run.sixteenths());
    let under = |r: Length| rise_at(r) - drop;
    let mut profile: Vec<(Length, Length)> = vec![(Length::ZERO, h - tt)];
    for i in 1..=treads {
        let seat = z(i) - tt;
        let front = run * i - back;
        profile.push(((run * (i - 1) - back).max(Length::ZERO), seat));
        profile.push((front, seat));
    }
    profile.push((run * treads - back, Length::ZERO));
    // Where the underside meets the pad, and the stringer's top end.
    let foot = Length::from_inches_f64(
        (under(Length::ZERO).as_inches_f64() * run.as_inches_f64() / riser.as_inches_f64())
            .max(0.0),
    )
    .min(run * treads - back);
    profile.push((foot, Length::ZERO));
    if under(Length::ZERO) > Length::ZERO {
        profile.push((Length::ZERO, under(Length::ZERO)));
    }
    profile.dedup();
    let throat =
        Length::from_inches_f64((drop - riser).as_inches_f64() * run.as_inches_f64() / slope_len);
    // The board a stringer is cut from: its length along the slope.
    let (cos, sin) = (
        run.as_inches_f64() / slope_len,
        riser.as_inches_f64() / slope_len,
    );
    let along: Vec<f64> = profile
        .iter()
        .map(|&(r, zz)| r.as_inches_f64() * cos - zz.as_inches_f64() * sin)
        .collect();
    let stringer_len = Length::from_inches_f64(
        along.iter().cloned().fold(f64::MIN, f64::max)
            - along.iter().cloned().fold(f64::MAX, f64::min),
    ) + Length::inches(1);
    let max_spacing = STRINGER_SPACING
        .min(decking_spacing(s.tread, DeckingMaterial::Wood).unwrap_or(STRINGER_SPACING));
    let count =
        ((s.width - st).sixteenths() + max_spacing.sixteenths() - 1) / max_spacing.sixteenths() + 1;
    let count = count.max(2);
    let spacing = (s.width - st).mul_div(1, count - 1);
    for k in 0..count {
        let a = s.at + (s.width - st).mul_div(k, count - 1);
        let label = format!("{pre}-ST{}", k + 1);
        let mut p = placed(
            label.clone(),
            MemberKind::Stringer,
            s.stringer,
            e.rect(side, (a, a + st), (Length::ZERO, run * treads - back)),
            false,
        );
        p.member.length = stringer_len;
        members.push(p);
        rises.push(Rise {
            label,
            lo: Length::ZERO,
            hi: h,
            profile: profile.iter().map(|&(r, zz)| (c(r), zz)).collect(),
            along_x,
        });
    }

    // Treads, two boards each, and riser boards when the rise is high.
    for i in 1..=treads {
        let r0 = run * (i - 1);
        for (j, letter) in ["a", "b"].iter().enumerate() {
            let r = r0 + (tw + gap) * j as i64;
            let label = format!("{pre}-T{i:02}{letter}");
            let mut p = placed(
                label.clone(),
                MemberKind::Tread,
                s.tread,
                e.rect(side, (s.at, s.at + s.width), (r, r + tw)),
                false,
            );
            p.member.length = s.width;
            members.push(p);
            rises.push(Rise::level(&label, z(i) - tt, z(i)));
        }
        if closed {
            let r = run * i - back;
            let label = format!("{pre}-RS{i:02}");
            let mut p = placed(
                label.clone(),
                MemberKind::Riser,
                RISER_BOARD,
                e.rect(side, (s.at, s.at + s.width), (r, r + back)),
                false,
            );
            p.member.length = s.width;
            members.push(p);
            rises.push(Rise::level(&label, z(i + 1), z(i) - tt));
        }
    }

    // Posts at the top and foot of each side for the handrail and guards.
    let handrail = risers >= 4;
    let guards = closed;
    let pw = POST.actual_width();
    let rt = RAIL.actual_thickness();
    let nose = |r: Length| h - riser.mul_div(r.sixteenths(), run.sixteenths());
    let mut top_posts = Vec::new();
    if handrail || guards {
        let foot_r = (run * treads - pw).max(pw);
        let sides_a = [(s.at - pw, false), (s.at + s.width, true)];
        let mut post_n = 0;
        let mut rail_n = 0;
        let mut bal_n = 0;
        for (k, &(pa, right)) in sides_a.iter().enumerate() {
            // Only one side carries a handrail without guards.
            if !guards && k == 1 {
                break;
            }
            let top_rect = e.rect(side, (pa, pa + pw), (Length::ZERO, pw));
            top_posts.push(top_rect);
            let shared = guard_posts.contains(&top_rect);
            let tops = [
                (Length::ZERO, h + GUARD_HEIGHT, shared, e.joist_bottom),
                (
                    foot_r,
                    nose(foot_r + half(pw)) + GUARD_HEIGHT,
                    false,
                    Length::ZERO,
                ),
            ];
            for (r, post_top, skip, post_bottom) in tops {
                if skip {
                    continue;
                }
                post_n += 1;
                let label = format!("{pre}-P{post_n}");
                let mut p = placed(
                    label.clone(),
                    MemberKind::GuardPost,
                    POST,
                    e.rect(side, (pa, pa + pw), (r, r + pw)),
                    false,
                );
                p.member.length = post_top - post_bottom;
                members.push(p);
                rises.push(Rise::level(&label, post_bottom, post_top));
            }
            // Rails sit on the posts' faces toward the stair.
            let ra = if right { pa - rt } else { pa + pw };
            let (r0, r1) = (pw, foot_r);
            let slope = |r: Length, above: Length| nose(r) + above;
            let rail_profile = |above: Length, depth: Length| {
                let v = Length::from_inches_f64(
                    depth.as_inches_f64() * slope_len / run.as_inches_f64(),
                );
                vec![
                    (c(r0), slope(r0, above)),
                    (c(r1), slope(r1, above)),
                    (c(r1), slope(r1, above) - v),
                    (c(r0), slope(r0, above) - v),
                ]
            };
            let sloped_len = Length::from_inches_f64(
                (r1 - r0).as_inches_f64() * slope_len / run.as_inches_f64(),
            ) + Length::inches(1);
            if guards {
                for above in [GUARD_HEIGHT, MAX_GAP + RAIL.actual_width()] {
                    rail_n += 1;
                    let label = format!("{pre}-R{rail_n}");
                    let mut p = placed(
                        label.clone(),
                        MemberKind::Rail,
                        RAIL,
                        e.rect(side, (ra, ra + rt), (r0, r1)),
                        false,
                    );
                    p.member.length = sloped_len;
                    members.push(p);
                    rises.push(Rise {
                        label,
                        lo: Length::ZERO,
                        hi: h + GUARD_HEIGHT,
                        profile: rail_profile(above, RAIL.actual_width()),
                        along_x,
                    });
                }
                // Balusters between the posts, on the rails' outer faces.
                let bw = BALUSTER.actual_width();
                let ba = if right { pa } else { pa + pw - bw };
                let clear = r1 - r0;
                let k = baluster_count(clear);
                for r in spread(r0, r1, bw, k) {
                    bal_n += 1;
                    let mid = r + half(bw);
                    let label = format!("{pre}-B{bal_n:02}");
                    let (lo, hi) = (nose(mid) + MAX_GAP, nose(mid) + GUARD_HEIGHT);
                    let mut p = placed(
                        label.clone(),
                        MemberKind::Baluster,
                        BALUSTER,
                        e.rect(side, (ba, ba + bw), (r, r + bw)),
                        false,
                    );
                    p.member.length = hi - lo;
                    members.push(p);
                    rises.push(Rise::level(&label, lo, hi));
                }
            }
            if handrail && k == 0 {
                // A graspable rail, 1-1/2" clear of the guard rail or post.
                let hw = HANDRAIL.actual_width();
                let ha = if guards {
                    pa + pw + rt + Length::from_sixteenths(24)
                } else {
                    pa + pw
                };
                let label = format!("{pre}-HR");
                let mut p = placed(
                    label.clone(),
                    MemberKind::Handrail,
                    HANDRAIL,
                    e.rect(side, (ha, ha + hw), (Length::ZERO, r1 + pw)),
                    false,
                );
                p.member.length = Length::from_inches_f64(
                    (r1 + pw).as_inches_f64() * slope_len / run.as_inches_f64(),
                ) + Length::inches(1);
                members.push(p);
                let v =
                    Length::from_inches_f64(hw.as_inches_f64() * slope_len / run.as_inches_f64());
                let end = r1 + pw;
                rises.push(Rise {
                    label,
                    lo: Length::ZERO,
                    hi: h + HANDRAIL_HEIGHT,
                    profile: vec![
                        (c(Length::ZERO), nose(Length::ZERO) + HANDRAIL_HEIGHT),
                        (c(end), nose(end) + HANDRAIL_HEIGHT),
                        (c(end), nose(end) + HANDRAIL_HEIGHT - v),
                        (c(Length::ZERO), nose(Length::ZERO) + HANDRAIL_HEIGHT - v),
                    ],
                    along_x,
                });
            }
        }
    }

    let extra = if handrail || guards { pw } else { Length::ZERO };
    let pad = e.rect(
        side,
        (s.at - extra, s.at + s.width + extra),
        (
            (run * treads - Length::inches(12)).max(Length::ZERO),
            run * treads + PAD_DEPTH,
        ),
    );
    Ok(FramedStairs {
        side,
        at: s.at,
        width: s.width,
        rise: h,
        risers,
        riser,
        run,
        total_run,
        tread: s.tread,
        tread_depth,
        stringer: s.stringer,
        stringers: count as usize,
        stringer_spacing: spacing,
        throat,
        handrail,
        guards,
        closed_risers: closed,
        pad,
        members,
        rises,
        top_posts,
    })
}

/// The ledger and what holds it to the house.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FramedLedger {
    pub size: Nominal,
    pub fasteners: LedgerFasteners,
    /// The joist span hung on it, which sets the spacing.
    pub joist_span: Length,
    /// The most the code allows between fasteners; `None` past its table.
    pub allowed: Option<Length>,
    /// Center to center between fasteners as laid out.
    pub spacing: Length,
    /// Each fastener's place along the ledger, and whether it's in the top
    /// row (they stagger, 2" from the top and bottom edges).
    pub bolts: Vec<(Length, bool)>,
}

pub fn frame_ledger(
    size: Nominal,
    fasteners: LedgerFasteners,
    width: Length,
    joist_span: Length,
) -> FramedLedger {
    let allowed = crate::code::ledger_spacing(fasteners, joist_span);
    let edge = Length::inches(2);
    let room = width - edge * 2;
    let step = allowed.unwrap_or(Length::inches(10));
    let gaps = ((room.sixteenths() + step.sixteenths() - 1) / step.sixteenths()).max(1);
    let bolts = (0..=gaps)
        .map(|k| (edge + room.mul_div(k, gaps), k % 2 == 0))
        .collect();
    FramedLedger {
        size,
        fasteners,
        joist_span,
        allowed,
        spacing: room.mul_div(1, gaps),
        bolts,
    }
}

/// Concrete to buy, in cubic inches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Concrete {
    pub footings: i64,
    pub footing_count: usize,
    pub pads: i64,
    pub pad_count: usize,
}

/// One 80 lb bag of concrete mix makes about 0.60 cubic feet, a 60 lb bag
/// about 0.45.
pub const BAG_80_CU_FT: f64 = 0.60;
pub const BAG_60_CU_FT: f64 = 0.45;
/// Extra for spillage and holes dug wide.
pub const CONCRETE_WASTE: f64 = 0.10;

impl Concrete {
    pub fn cubic_feet(&self) -> f64 {
        (self.footings + self.pads) as f64 / 1728.0
    }

    pub fn cubic_yards(&self) -> f64 {
        self.cubic_feet() / 27.0
    }

    /// Bags of each size to buy, waste included.
    pub fn bags(&self) -> (u32, u32) {
        let need = self.cubic_feet() * (1.0 + CONCRETE_WASTE);
        (
            (need / BAG_80_CU_FT).ceil() as u32,
            (need / BAG_60_CU_FT).ceil() as u32,
        )
    }

    /// `14.2 cu ft (0.53 cu yd): 26 bags of 80 lb or 35 of 60 lb, 10% extra included`.
    pub fn describe(&self) -> String {
        let (b80, b60) = self.bags();
        format!(
            "{:.1} cu ft ({:.2} cu yd): {b80} bags of 80 lb mix or {b60} of 60 lb, 10% extra included",
            self.cubic_feet(),
            self.cubic_yards()
        )
    }
}

/// A round column of concrete `diameter` across and `height` tall, in
/// cubic inches.
pub fn cylinder(diameter: Length, height: Length) -> i64 {
    let r = diameter.as_inches_f64() / 2.0;
    (std::f64::consts::PI * r * r * height.as_inches_f64()).ceil() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baluster_gaps_stay_under_four_inches() {
        for clear in [3, 4, 10, 30, 64, 68] {
            let c = Length::inches(clear);
            let k = baluster_count(c);
            let gap = (c - BALUSTER.actual_width() * k).mul_div(1, k + 1);
            assert!(gap <= MAX_GAP, "{clear}\": {k} balusters leave {gap}");
        }
    }

    #[test]
    fn concrete_bags() {
        let c = Concrete {
            footings: 1728 * 10,
            footing_count: 6,
            pads: 0,
            pad_count: 0,
        };
        // 11 cu ft with waste: 19 bags of 80 lb, 25 of 60 lb.
        assert_eq!(c.bags(), (19, 25));
    }
}
