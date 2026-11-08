//! A deck standing on its own footings: joists on center across beam
//! lines, the beams on piers or on posts, decking across the joists, and
//! every size checked against the deck rules the project picks.
//!
//! Plan coordinates: `x` along the beams from the left end, `y` along the
//! joists from the front edge. Heights are from the ground.

use serde::Serialize;
use sticks_core::{
    BeamStyle, Deck, DeckAttach, DeckSide, DeckSupport, GuardRule, Length, MemberKind, Nominal,
    Rules, Stock,
};

use crate::code::{
    check_deck, check_deck_parts, effective_joist_span, footings, longest_beam_span, BeamLoad,
    CodeCheck, DeckLayout, Footing, GUARD_ABOVE,
};
use crate::deck_parts::{
    cylinder, frame_guards, frame_ledger, frame_stairs, Concrete, Edge, FramedGuards, FramedLedger,
    FramedStairs, Rise, PAD_THICKNESS,
};
use crate::floor::Pier;
use crate::layout::{half, placed, LayoutMark, Placed, Rect, Splice};
use crate::platform::{
    beam_joints, beam_line, joints_over_joists, joist_layout, joist_marks, run_pieces,
    support_line, Decking,
};
use crate::FramingError;

/// Treated lumber this close to the ground should be rated for ground
/// contact (AWPA UC4A).
const GROUND_CONTACT: Length = Length::inches(6);

/// Shortest post worth standing between a pier and a beam.
const SHORTEST_POST: Length = Length::inches(6);

/// Heights through the deck, from the ground.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct DeckHeights {
    /// Top of the decking.
    pub top: Length,
    pub joist_top: Length,
    pub joist_bottom: Length,
    pub beam_top: Length,
    pub beam_bottom: Length,
    /// Tops of the piers or blocks the beams or posts stand on.
    pub pier_top: Length,
}

/// Everything generated for a deck.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FramedDeck {
    pub width: Length,
    pub depth: Length,
    pub heights: DeckHeights,
    /// Rims, joists and blocking, the beams, any posts, then the decking.
    pub members: Vec<Placed>,
    /// A pier (or block) under each support, beam by beam.
    pub piers: Vec<Pier>,
    /// The footing under each pier, matching `piers`.
    pub footings: Vec<Footing>,
    /// Each beam line's center, front to back.
    pub beam_y: Vec<Length>,
    /// Support centers along every beam.
    pub support_x: Vec<Length>,
    pub beam: Nominal,
    pub beam_plies: u8,
    pub beam_style: BeamStyle,
    pub support: DeckSupport,
    /// Joists, ordered from the layout end, as marked on the rims or beams.
    pub marks: Vec<LayoutMark>,
    pub splices: Vec<Splice>,
    /// Decking rows, front to back.
    pub rows: usize,
    pub layout: DeckLayout,
    /// The design checked against the deck rules.
    pub checks: Vec<CodeCheck>,
    pub warnings: Vec<String>,
    /// Guards along the open sides, when it has them.
    pub guards: Option<FramedGuards>,
    pub stairs: Vec<FramedStairs>,
    /// The ledger on the house, for a deck fastened to it.
    pub ledger: Option<FramedLedger>,
    /// Heights of guard and stair pieces, which don't sit at the framing's
    /// levels.
    pub rises: Vec<Rise>,
    /// Concrete for the footings and stair pads.
    pub concrete: Concrete,
}

impl FramedDeck {
    /// Whether every check passed.
    pub fn passes(&self) -> bool {
        self.checks.iter().all(|c| c.ok)
    }
}

pub fn frame_deck(deck: &Deck, rules: &Rules, stock: &[Stock]) -> Result<FramedDeck, FramingError> {
    let fail = |reason: String| FramingError {
        part: "deck".to_string(),
        reason,
    };
    let joist = deck.joist;
    let t = joist.actual_thickness();
    let (width, depth) = (deck.width, deck.depth);
    let oc = deck.on_center.unwrap_or(rules.on_center);
    let flush = deck.beam_style == BeamStyle::Flush;
    if oc < t * 2 {
        return Err(fail(format!(
            "joists {oc:#} on center leaves no room between joists"
        )));
    }
    if width < t * 4 || depth < t * 4 {
        return Err(fail(format!("{width} by {depth} is too small to frame")));
    }
    let ledger = deck.attach == DeckAttach::Ledger;
    if deck.beams < 1 || (!ledger && deck.beams < 2) {
        return Err(fail(if ledger {
            "needs at least one beam line under the joists' outer ends".to_string()
        } else {
            "needs at least two beam lines, one near each end of the joists".to_string()
        }));
    }
    if deck.beam_plies == 0 {
        return Err(fail("a beam needs at least one ply".to_string()));
    }
    if deck.overhang < Length::ZERO || deck.gap < Length::ZERO {
        return Err(fail("the overhang and gap can't be negative".to_string()));
    }
    if flush && deck.overhang > Length::ZERO {
        return Err(fail(
            "joists hung between flush beams can't run past them; set the overhang to 0 or put the beams under the joists".to_string(),
        ));
    }
    if deck.post_inset < Length::ZERO || deck.post_inset * 2 >= width {
        return Err(fail(format!(
            "posts set in {} from each end leave no room along a {width} beam",
            deck.post_inset
        )));
    }
    let ply = deck.beam.actual_thickness();
    let bw = ply * i64::from(deck.beam_plies);
    let n = i64::from(deck.beams);
    let reach = if ledger {
        depth - deck.overhang - t
    } else {
        depth - deck.overhang * 2
    };
    if bw * n > reach {
        return Err(fail(format!(
            "{} beams {bw:#} wide don't fit in {depth} of joists",
            deck.beams
        )));
    }

    // Heights, from the top of the decking down.
    let top = deck.height;
    let joist_top = top - deck.decking.actual_thickness();
    let joist_bottom = joist_top - joist.actual_width();
    let beam_top = if flush { joist_top } else { joist_bottom };
    let beam_bottom = beam_top - deck.beam.actual_width();
    let lowest = deck.pier_above_grade;
    let too_low = |what: &str| {
        let hint = if flush {
            "use smaller lumber or raise the deck"
        } else {
            "hang the joists between flush beams, use smaller lumber or raise the deck"
        };
        fail(format!(
            "at {top} the beams' bottoms are {beam_bottom} off the ground, below the {lowest:#} {what}; {hint}"
        ))
    };
    let (pier_top, post_len) = match deck.support {
        DeckSupport::Piers => {
            if beam_bottom < lowest {
                return Err(too_low("pier tops"));
            }
            (beam_bottom, None)
        }
        DeckSupport::Posts | DeckSupport::Blocks => {
            let len = beam_bottom - lowest;
            if len < SHORTEST_POST {
                return Err(if len < Length::ZERO {
                    too_low("tops of the footings")
                } else {
                    fail(format!(
                        "posts would be only {len:#} long; sit the beams straight on piers instead"
                    ))
                });
            }
            (lowest, Some(len))
        }
    };

    // Beam lines: the outer two at the ends of the joists, less any
    // overhang, the rest evenly between, by their centers. On a ledger deck
    // the ledger's face is the back bearing, and the beams share out the
    // room in front of it.
    let first = deck.overhang;
    let ledger_face = depth - t;
    let beam_face: Vec<Length> = if ledger {
        let front = first + half(bw);
        (0..n)
            .map(|i| front + (ledger_face - front).mul_div(i, n) - half(bw))
            .collect()
    } else {
        let last = depth - deck.overhang - bw;
        (0..n)
            .map(|i| first + (last - first).mul_div(i, n - 1))
            .collect()
    };
    let beam_y: Vec<Length> = beam_face.iter().map(|&y| y + half(bw)).collect();
    let mut bearings = beam_y.clone();
    if ledger {
        bearings.push(ledger_face);
    }
    let joist_spans: Vec<Length> = bearings.windows(2).map(|w| w[1] - w[0]).collect();
    let overhangs = match (flush, ledger) {
        (true, _) => [Length::ZERO, Length::ZERO],
        (false, true) => [beam_y[0], Length::ZERO],
        (false, false) => [beam_y[0], depth - beam_y[beam_y.len() - 1]],
    };

    // What each beam carries: the joist span on each side, or the overhang
    // where there's no span.
    let count = beam_y.len();
    let mut loads: Vec<(String, Length, Length)> = Vec::new();
    for i in 0..count {
        let name = match i {
            0 if count == 1 => "beam".to_string(),
            0 => "front beam".to_string(),
            i if i == count - 1 && !ledger => "back beam".to_string(),
            i => format!("beam {}", i + 1),
        };
        let front = (i > 0).then(|| joist_spans[i - 1]);
        let back = joist_spans.get(i).copied();
        let (influence, effective) = match (front, back) {
            (Some(a), Some(b)) => (half(a + b), a + b),
            (None, Some(span)) => (
                half(span) + overhangs[0],
                effective_joist_span(span, overhangs[0]),
            ),
            (Some(span), None) => (
                half(span) + overhangs[1],
                effective_joist_span(span, overhangs[1]),
            ),
            (None, None) => unreachable!("every beam has a span on one side"),
        };
        loads.push((name, influence, effective));
    }

    // Supports along the beams, as far apart as the rules allow unless the
    // project says how far.
    let spacing = match deck.post_spacing {
        Some(s) if s > Length::ZERO => s,
        Some(_) => return Err(fail("post spacing must be more than zero".to_string())),
        None => loads
            .iter()
            .filter_map(|(_, inf, eff)| longest_beam_span(deck, *inf, *eff))
            .min()
            .unwrap_or(Length::feet(6)),
    };
    let support_x = support_line(width, deck.post_inset, spacing);
    let gaps: Vec<Length> = support_x.windows(2).map(|w| w[1] - w[0]).collect();
    let beam_span = gaps.iter().copied().max().unwrap_or_default();
    let shares: Vec<Length> = (0..support_x.len())
        .map(|k| {
            let left = if k == 0 {
                deck.post_inset * 2
            } else {
                gaps[k - 1]
            };
            let right = if k == support_x.len() - 1 {
                deck.post_inset * 2
            } else {
                gaps[k]
            };
            half(left + right)
        })
        .collect();
    let beams: Vec<BeamLoad> = loads
        .into_iter()
        .map(|(name, influence, effective)| BeamLoad {
            name,
            influence,
            effective,
            span: beam_span,
            end_overhang: deck.post_inset,
            shares: shares.clone(),
        })
        .collect();

    let mut members = Vec::new();
    let mut splices = Vec::new();
    let mut warnings = Vec::new();
    let (joist_x, centers) = joist_layout(width, oc, t, rules.layout_from);

    // The ledger along the back, in place of a rim, at least a 2x8.
    let ledger_size = if joist.actual_width() < Nominal::new(2, 8).actual_width() {
        joist.with_width(8)
    } else {
        joist
    };
    if ledger {
        let joints = joints_over_joists("ledger", ledger_size, width, &centers, stock, rules)
            .map_err(fail)?;
        members.extend(run_pieces(
            "deck-LG",
            1,
            MemberKind::Ledger,
            ledger_size,
            width,
            ledger_face,
            t,
            &joints,
        ));
        splices.extend(joints.iter().map(|&at| Splice {
            kind: MemberKind::Ledger,
            at,
        }));
    }
    if flush {
        // Joists hang between each pair of beams; the outer beams are the
        // deck's front and back edges, or the ledger is its back.
        let mut faces: Vec<(Length, Length)> = beam_face.iter().map(|&y| (y, y + bw)).collect();
        if ledger {
            faces.push((ledger_face, depth));
        }
        let bays = faces.len() - 1;
        for (i, &x) in joist_x.iter().enumerate() {
            for b in 0..bays {
                let (a, z) = (faces[b].1, faces[b + 1].0);
                let mut label = format!("deck-J{:02}", i + 1);
                if bays > 1 {
                    label.push((b'a' + b as u8) as char);
                }
                members.push(placed(
                    label,
                    MemberKind::Joist,
                    joist,
                    Rect {
                        x,
                        y: a,
                        w: t,
                        h: z - a,
                    },
                    true,
                ));
            }
        }
    } else {
        // Rims across the joists' ends, joists running between them over
        // the beams, and blocking between joists over any inner beam.
        let rim_joints =
            joints_over_joists("rim", joist, width, &centers, stock, rules).map_err(fail)?;
        let mut rim_count = 0;
        let rim_ys: &[Length] = if ledger {
            &[Length::ZERO]
        } else {
            &[Length::ZERO, depth - t]
        };
        for &y in rim_ys {
            let rims = run_pieces(
                "deck-RJ",
                rim_count + 1,
                MemberKind::RimJoist,
                joist,
                width,
                y,
                t,
                &rim_joints,
            );
            rim_count += rims.len();
            members.extend(rims);
            splices.extend(rim_joints.iter().map(|&at| Splice {
                kind: MemberKind::RimJoist,
                at,
            }));
        }
        for (i, &x) in joist_x.iter().enumerate() {
            members.push(placed(
                format!("deck-J{:02}", i + 1),
                MemberKind::Joist,
                joist,
                Rect {
                    x,
                    y: t,
                    w: t,
                    h: depth - t * 2,
                },
                true,
            ));
        }
        let mut blocks = 0;
        let inner = if ledger {
            &beam_y[1..]
        } else {
            &beam_y[1..beam_y.len() - 1]
        };
        for &y in inner {
            for pair in joist_x.windows(2) {
                blocks += 1;
                members.push(placed(
                    format!("deck-B{blocks:02}"),
                    MemberKind::Blocking,
                    joist,
                    Rect {
                        x: pair[0] + t,
                        y: y - half(t),
                        w: pair[1] - pair[0] - t,
                        h: t,
                    },
                    false,
                ));
            }
        }
    }

    // Beams, each ply spliced over a support, the plies' joints staggered.
    let (ply_joints, shared) = beam_joints(
        deck.beam,
        deck.beam_plies,
        width,
        &support_x,
        "post",
        stock,
        rules,
    )
    .map_err(fail)?;
    if shared {
        warnings.push(
            "the beam plies are spliced over the same post; closer posts would let the joints stagger"
                .to_string(),
        );
    }
    for (i, &y) in beam_face.iter().enumerate() {
        let (beam, beam_splices) = beam_line(
            &format!("deck-BM{}", i + 1),
            MemberKind::Beam,
            deck.beam,
            width,
            y,
            &ply_joints,
        );
        members.extend(beam);
        splices.extend(beam_splices);
    }

    // Piers, or blocks, under every support, and posts on them.
    let piers: Vec<Pier> = beam_y
        .iter()
        .flat_map(|&y| support_x.iter().map(move |&x| Pier { x, y }))
        .collect();
    if let Some(len) = post_len {
        let (pw, pd) = (deck.post.actual_width(), deck.post.actual_thickness());
        for (i, pier) in piers.iter().enumerate() {
            let mut p = placed(
                format!("deck-P{:02}", i + 1),
                MemberKind::Post,
                deck.post,
                Rect {
                    x: pier.x - half(pw),
                    y: pier.y - half(pd),
                    w: pw,
                    h: pd,
                },
                false,
            );
            p.member.length = len;
            members.push(p);
        }
    }

    // Decking across the joists, front to back.
    let laid = Decking {
        prefix: "deck-DK",
        width,
        y0: Length::ZERO,
        y1: depth,
        centers: &centers,
        board: deck.decking,
        gap: deck.gap,
        oc,
        rip_at: "at the back",
    }
    .lay(stock, rules)
    .map_err(fail)?;
    members.extend(laid.members);
    warnings.extend(laid.warnings);

    if beam_bottom < GROUND_CONTACT
        || (deck.support == DeckSupport::Piers && pier_top < GROUND_CONTACT)
    {
        warnings.push(format!(
            "the beams are within {GROUND_CONTACT:#} of the ground; buy treated lumber rated for ground contact (UC4A) for them"
        ));
    }
    splices.sort_by_key(|s| s.at);
    splices.dedup();

    let marks = joist_marks("deck-J", &joist_x, t, width, rules.layout_from);
    let layout = DeckLayout {
        oc,
        joist_spans,
        overhangs,
        beams,
        post_height: post_len,
    };
    let post_width = match post_len {
        Some(_) => deck.post.actual_width(),
        None => bw,
    };
    let footing_list: Vec<Footing> = layout
        .beams
        .iter()
        .flat_map(|b| footings(deck, b, post_width))
        .collect();
    let mut checks = check_deck(deck, &layout);

    // Guards on the open sides, stairs, and the ledger's fasteners.
    let edge = Edge {
        deck,
        width,
        depth,
        top,
        joist_bottom,
        stock,
        rules,
    };
    let open: Vec<DeckSide> = [
        DeckSide::Front,
        DeckSide::Left,
        DeckSide::Right,
        DeckSide::Back,
    ]
    .into_iter()
    .filter(|&s| !(ledger && s == DeckSide::Back))
    .collect();
    let want_guards = match deck.guards {
        GuardRule::Auto => top > GUARD_ABOVE,
        GuardRule::Always => true,
        GuardRule::Never => false,
    };
    let guards = if want_guards {
        let g = frame_guards(&edge, &open).map_err(fail)?;
        members.extend(g.members.iter().cloned());
        Some(g)
    } else {
        None
    };
    let guard_posts: Vec<Rect> = guards
        .iter()
        .flat_map(|g| &g.members)
        .filter(|p| p.member.kind == MemberKind::GuardPost)
        .map(|p| p.rect)
        .collect();
    let mut stairs = Vec::new();
    for (i, s) in deck.stairs.iter().enumerate() {
        if ledger && s.side == DeckSide::Back {
            return Err(fail(
                "stairs can't leave from the back; that's the ledger on the house".to_string(),
            ));
        }
        let st = frame_stairs(&edge, i + 1, s, &guard_posts).map_err(fail)?;
        members.extend(st.members.iter().cloned());
        stairs.push(st);
    }
    let ledger_parts = ledger.then(|| {
        frame_ledger(
            ledger_size,
            deck.ledger_fasteners,
            width,
            layout.joist_spans.last().copied().unwrap_or_default(),
        )
    });
    checks.extend(check_deck_parts(
        deck,
        guards.as_ref(),
        &stairs,
        ledger_parts.as_ref(),
    ));
    let (mut lo, mut hi) = ((Length::ZERO, Length::ZERO), (width, depth));
    for r in members
        .iter()
        .map(|p| p.rect)
        .chain(stairs.iter().map(|s| s.pad))
    {
        lo = (lo.0.min(r.x), lo.1.min(r.y));
        hi = (hi.0.max(r.right()), hi.1.max(r.top()));
    }
    checks.extend(crate::code::check_site(deck, lo, hi));
    let mut rises: Vec<Rise> = guards.iter().flat_map(|g| g.rises.clone()).collect();
    rises.extend(stairs.iter().flat_map(|s| s.rises.clone()));

    // Concrete: each footing a round column from its bottom up to the pier
    // top, and a pad at the foot of each stair.
    let concrete = Concrete {
        footings: footing_list
            .iter()
            .map(|f| cylinder(f.diameter, f.depth + pier_top))
            .sum(),
        footing_count: footing_list.len(),
        pads: stairs
            .iter()
            .map(|s| {
                let (w, d) = (s.pad.w.as_inches_f64(), s.pad.h.as_inches_f64());
                (w * d * PAD_THICKNESS.as_inches_f64()).ceil() as i64
            })
            .sum(),
        pad_count: stairs.len(),
    };
    let concrete = if deck.support == DeckSupport::Blocks {
        Concrete {
            footings: 0,
            footing_count: 0,
            ..concrete
        }
    } else {
        concrete
    };

    Ok(FramedDeck {
        width,
        depth,
        heights: DeckHeights {
            top,
            joist_top,
            joist_bottom,
            beam_top,
            beam_bottom,
            pier_top,
        },
        members,
        piers,
        footings: footing_list,
        beam_y,
        support_x,
        beam: deck.beam,
        beam_plies: deck.beam_plies,
        beam_style: deck.beam_style,
        support: deck.support,
        marks,
        splices,
        rows: laid.rows,
        layout,
        checks,
        warnings,
        guards,
        stairs,
        ledger: ledger_parts,
        rises,
        concrete,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sticks_core::{DeckCode, Project};

    fn l(s: &str) -> Length {
        s.parse().unwrap()
    }

    fn deck(extra: &str) -> Result<FramedDeck, FramingError> {
        let p = Project::from_toml(&format!("name = \"Deck\"\n[deck]\n{extra}")).unwrap();
        frame_deck(p.deck.as_ref().unwrap(), &p.rules, &p.stock)
    }

    fn of_kind(d: &FramedDeck, kind: MemberKind) -> Vec<&Placed> {
        d.members.iter().filter(|p| p.member.kind == kind).collect()
    }

    fn failing(d: &FramedDeck) -> Vec<String> {
        d.checks
            .iter()
            .filter(|c| !c.ok)
            .map(|c| format!("{}: {} vs {}", c.item, c.actual, c.allowed))
            .collect()
    }

    /// The default: 12' square, decking 18" up, 2x8 joists at 16" on two
    /// beams of (2) 2x8 under them, straight on piers.
    #[test]
    fn frames_the_default_deck() {
        let d = deck("").unwrap();
        // 1" decking, 7-1/4" joists and beams: beams bottom out 2-1/2" up.
        assert_eq!(d.heights.joist_top, l("17"));
        assert_eq!(d.heights.joist_bottom, l("9-3/4"));
        assert_eq!(d.heights.beam_bottom, l("2-1/2"));
        assert_eq!(d.heights.pier_top, l("2-1/2"));
        // Beams flush with the ends: 141" between their centers.
        assert_eq!(d.beam_y, [l("1-1/2"), l("142-1/2")]);
        assert_eq!(d.layout.joist_spans, [l("141")]);
        let joists = of_kind(&d, MemberKind::Joist);
        assert_eq!(joists.len(), 10);
        assert!(joists.iter().all(|j| j.member.length == l("141")));
        assert_eq!(of_kind(&d, MemberKind::RimJoist).len(), 2);
        // Each beam carries 70-1/2" of joists, an effective span under 12':
        // (2) 2x8 southern pine spans 6' 2", so three piers a beam.
        assert_eq!(d.support_x, [l("12"), l("72"), l("132")]);
        assert_eq!(d.piers.len(), 6);
        assert_eq!(of_kind(&d, MemberKind::Beam).len(), 4);
        assert!(of_kind(&d, MemberKind::Post).is_empty());
        // 144 / (5-1/2 + 1/8) = 25.6 rows.
        assert_eq!(d.rows, 26);
        assert!(d.passes(), "{:?}", failing(&d));
    }

    #[test]
    fn a_long_joist_span_fails_its_check() {
        let d = deck("depth = \"14'\"").unwrap();
        let bad = failing(&d);
        assert!(bad.iter().any(|c| c.starts_with("Joist span")), "{bad:?}");
    }

    #[test]
    fn flush_beams_hang_the_joists_for_a_low_deck() {
        // At 12" there's no room for beams under 2x8 joists.
        let err = deck("height = \"12\"").unwrap_err();
        assert!(err.reason.contains("flush"), "{err}");
        let d = deck("height = \"12\"\nbeam_style = \"flush\"\nbeam = \"2x8\"").unwrap();
        assert_eq!(d.heights.beam_bottom, l("3-3/4"));
        assert!(of_kind(&d, MemberKind::RimJoist).is_empty());
        let joists = of_kind(&d, MemberKind::Joist);
        // Between the beams' inside faces.
        assert!(joists.iter().all(|j| j.member.length == l("138")));
        assert!(
            d.warnings.iter().any(|w| w.contains("ground contact")),
            "{:?}",
            d.warnings
        );
    }

    #[test]
    fn posts_stand_on_piers_when_asked() {
        let d = deck("height = \"30\"\nsupport = \"posts\"\npost = \"6x6\"").unwrap();
        let posts = of_kind(&d, MemberKind::Post);
        assert_eq!(posts.len(), d.piers.len());
        // 30 - 1 - 7-1/4 - 7-1/4 - 2 = 12-1/2".
        assert!(posts.iter().all(|p| p.member.length == l("12-1/2")));
    }

    #[test]
    fn fairfax_checks_its_own_tables() {
        let d = deck("code = \"fairfax2021\"\nheight = \"28\"\nsupport = \"posts\"").unwrap();
        assert!(d.passes(), "{:?}", failing(&d));
        let footing = d.checks.iter().find(|c| c.item == "Footing depth").unwrap();
        assert!(footing.actual.contains("24"), "{footing:?}");
        let _ = DeckCode::Fairfax2021;
        // Deck blocks aren't in the county's details.
        let blocks = deck("code = \"fairfax2021\"\nheight = \"24\"\nsupport = \"blocks\"").unwrap();
        assert!(failing(&blocks).iter().any(|c| c.starts_with("Footings")));
    }

    #[test]
    fn overhangs_move_the_beams_in() {
        let d = deck("overhang = \"18\"\ndepth = \"14'\"").unwrap();
        assert_eq!(d.beam_y[0], l("19-1/2"));
        assert_eq!(d.layout.overhangs[0], l("19-1/2"));
        assert!(d.passes(), "{:?}", failing(&d));
    }
}

#[cfg(test)]
mod parts_tests {
    use super::*;
    use sticks_core::Project;

    fn l(s: &str) -> Length {
        s.parse().unwrap()
    }

    fn deck(extra: &str) -> FramedDeck {
        let p = Project::from_toml(&format!("name = \"Deck\"\n[deck]\n{extra}")).unwrap();
        frame_deck(p.deck.as_ref().unwrap(), &p.rules, &p.stock).unwrap()
    }

    fn count(d: &FramedDeck, kind: MemberKind) -> usize {
        d.members.iter().filter(|p| p.member.kind == kind).count()
    }

    fn failing(d: &FramedDeck) -> Vec<String> {
        d.checks
            .iter()
            .filter(|c| !c.ok)
            .map(|c| format!("{}: {} vs {}", c.item, c.actual, c.allowed))
            .collect()
    }

    #[test]
    fn a_low_deck_has_no_guards() {
        let d = deck("height = \"28\"");
        assert!(d.guards.is_none());
        assert_eq!(count(&d, MemberKind::GuardPost), 0);
        assert!(d.passes(), "{:?}", failing(&d));
    }

    /// 36" up, 12' square: guards on all four sides, posts no more than 6'
    /// apart, and no gap a 4" ball gets through.
    #[test]
    fn a_high_deck_gets_guards_all_round() {
        let d = deck("height = \"36\"\nsupport = \"posts\"");
        let g = d.guards.as_ref().unwrap();
        assert_eq!(g.sides.len(), 4);
        assert!(g.post_spacing <= l("6'"));
        assert!(g.gap < l("4"), "{}", g.gap);
        // Three posts a side: two bays of about 5' 10" on a 12' side.
        assert_eq!(g.posts, 12);
        // Posts run from the bottom of the rim to 36" over the decking.
        let post = d
            .members
            .iter()
            .find(|p| p.member.kind == MemberKind::GuardPost)
            .unwrap();
        assert_eq!(
            post.member.length,
            l("36") + l("36") - d.heights.joist_bottom
        );
        assert!(d.passes(), "{:?}", failing(&d));
        let never = deck("height = \"36\"\nsupport = \"posts\"\nguards = \"never\"");
        assert!(failing(&never).iter().any(|c| c.starts_with("Guards")));
    }

    /// 28" up: four risers of 7", three treads of two 2x6s, three 2x12
    /// stringers for a 36" stair, a handrail, and a pad at the foot.
    #[test]
    fn stairs_down_from_a_low_deck() {
        let d = deck("height = \"28\"\n[[deck.stairs]]\nside = \"front\"\nat = \"4'\"");
        let s = &d.stairs[0];
        assert_eq!(s.risers, 4);
        assert_eq!(s.riser, l("7"));
        assert_eq!(s.run, l("10-1/4"));
        assert_eq!(s.total_run, l("30-3/4"));
        assert_eq!(s.stringers, 3);
        assert!(s.handrail && !s.guards && !s.closed_risers);
        assert!(s.throat >= l("5"), "{}", s.throat);
        assert_eq!(count(&d, MemberKind::Stringer), 3);
        assert_eq!(count(&d, MemberKind::Tread), 6);
        assert_eq!(count(&d, MemberKind::Handrail), 1);
        // The pad starts under the stringers' feet and runs 36" out.
        assert_eq!(s.pad.y, -(s.total_run + l("36")));
        assert!(d.concrete.pads > 0);
        assert!(d.passes(), "{:?}", failing(&d));
    }

    /// A high stair shares the deck's guard posts at its top and gets guards
    /// and closed risers of its own.
    #[test]
    fn high_stairs_get_guards() {
        let d = deck(
            "height = \"40\"\nsupport = \"posts\"\npost = \"6x6\"\n[[deck.stairs]]\nside = \"left\"\nat = \"3'\"",
        );
        let s = &d.stairs[0];
        assert_eq!(s.risers, 6);
        assert!(s.guards && s.closed_risers);
        // Top posts were already the deck's, so only the two at the foot.
        let posts = s
            .members
            .iter()
            .filter(|p| p.member.kind == MemberKind::GuardPost)
            .count();
        assert_eq!(posts, 2);
        assert_eq!(count(&d, MemberKind::Riser), 5);
        assert!(d.passes(), "{:?}", failing(&d));
    }

    /// Fastened to the house: one beam out front, the joists hung from a
    /// 2x8 ledger, its lag screws spaced by the IRC table.
    #[test]
    fn a_ledger_deck() {
        let d = deck(
            "attach = \"ledger\"\nbeams = 1\nheight = \"28\"\noverhang = \"12\"\ndepth = \"10'\"",
        );
        assert_eq!(d.beam_y.len(), 1);
        assert_eq!(count(&d, MemberKind::Ledger), 1);
        assert_eq!(count(&d, MemberKind::RimJoist), 1);
        let ledger = d.ledger.as_ref().unwrap();
        // Beam at 12" + 1-1/2", ledger face at 118-1/2": 8' 9" span, so
        // lags at 18" o.c. at most.
        assert_eq!(ledger.joist_span, l("105"));
        assert_eq!(ledger.allowed, Some(l("18")));
        assert!(ledger.spacing <= l("18"));
        assert_eq!(d.layout.overhangs, [l("13-1/2"), Length::ZERO]);
        assert!(d.passes(), "{:?}", failing(&d));
        assert!(!d.checks.iter().any(|c| c.item == "Lateral bracing"));
    }

    #[test]
    fn concrete_adds_up() {
        let d = deck("");
        // Six 12" round footings... at least something for each.
        assert_eq!(d.concrete.footing_count, 6);
        assert!(d.concrete.cubic_feet() > 1.0);
    }
}
