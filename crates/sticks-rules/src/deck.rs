//! A deck standing on its own footings: joists on center across beam
//! lines, the beams on piers or on posts, decking across the joists, and
//! every size checked against the deck rules the project picks.
//!
//! Plan coordinates: `x` along the beams from the left end, `y` along the
//! joists from the front edge. Heights are from the ground.

use serde::Serialize;
use sticks_core::{BeamStyle, Deck, DeckSupport, Length, MemberKind, Nominal, Rules, Stock};

use crate::code::{
    check_deck, effective_joist_span, footings, longest_beam_span, BeamLoad, CodeCheck, DeckLayout,
    Footing,
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
    if deck.beams < 2 {
        return Err(fail(
            "needs at least two beam lines, one near each end of the joists".to_string(),
        ));
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
    let reach = depth - deck.overhang * 2;
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
    // overhang; the rest evenly between, by their centers.
    let first = deck.overhang;
    let last = depth - deck.overhang - bw;
    let beam_face: Vec<Length> = (0..n)
        .map(|i| first + (last - first).mul_div(i, n - 1))
        .collect();
    let beam_y: Vec<Length> = beam_face.iter().map(|&y| y + half(bw)).collect();
    let joist_spans: Vec<Length> = beam_y.windows(2).map(|w| w[1] - w[0]).collect();
    let overhangs = if flush {
        [Length::ZERO, Length::ZERO]
    } else {
        [beam_y[0], depth - beam_y[beam_y.len() - 1]]
    };

    // What each beam carries.
    let count = beam_y.len();
    let mut loads: Vec<(String, Length, Length)> = Vec::new();
    for i in 0..count {
        let name = match i {
            0 => "front beam".to_string(),
            i if i == count - 1 => "back beam".to_string(),
            i => format!("beam {}", i + 1),
        };
        let (influence, effective) = if i == 0 || i == count - 1 {
            let back = if i == 0 {
                joist_spans[0]
            } else {
                joist_spans[count - 2]
            };
            let over = overhangs[usize::from(i != 0)];
            (half(back) + over, effective_joist_span(back, over))
        } else {
            let (a, b) = (joist_spans[i - 1], joist_spans[i]);
            (half(a + b), a + b)
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

    if flush {
        // Joists hang between each pair of beams; the outer beams are the
        // deck's front and back edges.
        let bays = beam_face.len() - 1;
        for (i, &x) in joist_x.iter().enumerate() {
            for b in 0..bays {
                let (a, z) = (beam_face[b] + bw, beam_face[b + 1]);
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
        for y in [Length::ZERO, depth - t] {
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
        for &y in &beam_y[1..beam_y.len() - 1] {
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
    let checks = check_deck(deck, &layout);

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
