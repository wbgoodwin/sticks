//! A covered porch across the front of the building: posts on piers, a
//! built-up beam on the posts, and a roof of rafters hung on a ledger bolted
//! to the front wall, sloping down to the beam with a birdsmouth over it.
//!
//! With a deck, the posts stand on it over a beam on piers, and its joists
//! hang from a ledger on the floor's front rim.
//!
//! Plan coordinates match the floor and roof: `x` along the front from the
//! left end, `y` from the outside face of the front wall toward the back, so
//! the porch sits at negative `y`. Heights are from the top of the subfloor.

use serde::Serialize;
use sticks_core::{
    longest_board, Building, Cut, Floor, LayoutStart, Length, MemberKind, Nominal, Pitch, Porch,
    PorchDeck, Roof, Rules, Stock,
};

use crate::floor::Pier;
use crate::layout::{
    half, on_center, pieces, placed, ply_joints, split_run, LayoutMark, Placed, Rect, Splice,
};
use crate::platform::{
    beam_joints, beam_line, joints_over_joists, joist_layout, joist_marks, run_pieces, Decking,
};
use crate::roof::{two_up, RafterDetail, RafterTop, RoofPlane};
use crate::FramingError;

/// Clear height under the beam that's comfortable to walk under.
const HEADROOM: Length = Length::inches(80);

/// Heights through the porch, from the top of the subfloor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct PorchHeights {
    /// Top of the ledger, where the rafters' top edges start.
    pub ledger_top: Length,
    pub beam_top: Length,
    pub beam_bottom: Length,
    /// Where the posts stand: the tops of the piers.
    pub post_bottom: Length,
}

/// Heights through the deck, from the top of the subfloor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct PorchDeckHeights {
    /// Top of the decking.
    pub top: Length,
    pub joist_top: Length,
    pub joist_bottom: Length,
    /// The beam's bottom: the tops of the deck's piers.
    pub beam_bottom: Length,
}

/// Everything generated for the porch deck.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FramedPorchDeck {
    pub heights: PorchDeckHeights,
    /// Ledger, joists, rim, beam and decking, in plan.
    pub members: Vec<Placed>,
    /// A pier under the beam at each post.
    pub piers: Vec<Pier>,
    /// Joists, ordered from the layout end, as marked on the ledger.
    pub marks: Vec<LayoutMark>,
    pub splices: Vec<Splice>,
    /// Decking rows, front edge to the shed.
    pub rows: usize,
    pub warnings: Vec<String>,
}

/// Everything generated for the porch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FramedPorch {
    pub width: Length,
    /// From the front wall to the outside of the posts.
    pub depth: Length,
    pub heights: PorchHeights,
    /// Posts, beam plies, ledger, rafters and fascia, in plan.
    pub members: Vec<Placed>,
    pub rafter: RafterDetail,
    /// A pier under each post.
    pub piers: Vec<Pier>,
    /// Rafters, ordered from the layout end, as marked on the ledger.
    pub marks: Vec<LayoutMark>,
    pub splices: Vec<Splice>,
    pub plane: RoofPlane,
    pub deck: Option<FramedPorchDeck>,
    pub warnings: Vec<String>,
}

/// Frames `porch` across the front of `building`, its posts standing on the
/// deck, or else on piers level with the floor's or on the slab.
pub fn frame_porch(
    porch: &Porch,
    building: &Building,
    roof: Option<&Roof>,
    floor: Option<&Floor>,
    rules: &Rules,
    stock: &[Stock],
) -> Result<FramedPorch, FramingError> {
    let fail = |reason: String| FramingError {
        part: "porch".to_string(),
        reason,
    };
    let deck = match (&porch.deck, floor) {
        (Some(d), Some(f)) => Some(frame_deck(d, porch, building, f, rules, stock)?),
        (Some(_), None) => {
            return Err(fail(
                "a porch deck hangs from the floor's rim; add a floor or leave the deck out"
                    .to_string(),
            ))
        }
        _ => None,
    };
    let post_bottom = match (&deck, floor) {
        (Some(d), _) => d.heights.top,
        (None, Some(f)) => -(f.subfloor + f.joist.actual_width() + f.beam.actual_width()),
        (None, None) => Length::ZERO,
    };
    let pitch: Pitch = porch
        .pitch
        .or(roof.map(|r| r.pitch))
        .ok_or_else(|| fail("give the porch a pitch, or the building a roof".to_string()))?;
    let size = porch
        .rafter
        .or(roof.map(|r| r.rafter))
        .unwrap_or(sticks_core::Nominal::new(2, 6));
    let ledger = porch.ledger.unwrap_or(two_up(size));
    let width = building.width;
    let t = size.actual_thickness();
    let lt = ledger.actual_thickness();
    let oc = roof.and_then(|r| r.on_center).unwrap_or(rules.on_center);
    let (post_w, post_d) = (porch.post.actual_width(), porch.post.actual_thickness());
    if porch.beam_plies == 0 {
        return Err(fail("a beam needs at least one ply".to_string()));
    }
    let ply = porch.beam.actual_thickness();
    let beam_w = ply * i64::from(porch.beam_plies);
    if porch.overhang < Length::ZERO {
        return Err(fail("the overhang can't be negative".to_string()));
    }
    if porch.depth <= lt + beam_w.max(post_d) + t {
        return Err(fail(format!("{} is too shallow for a porch", porch.depth)));
    }
    if porch.post_spacing <= Length::ZERO {
        return Err(fail("post spacing must be more than zero".to_string()));
    }
    if width < post_w * 2 || oc < t * 2 {
        return Err(fail(format!("{width} is too narrow for a porch")));
    }

    // The ledger hangs just under the front wall's top plates by default.
    let plate = rules.wall_lumber.actual_thickness();
    let ledger_top = porch.ledger_height.unwrap_or(building.height - plate * 2);
    let mut warnings = Vec::new();
    if ledger_top > building.height - plate * 2 {
        warnings.push(format!(
            "the ledger's top at {ledger_top} is above the front wall's studs; it needs blocking or a rim to bolt to"
        ));
    }

    // Rafters: top edge flush with the ledger's top at its face, sloping
    // down to a birdsmouth over the beam's inside corner.
    let d_rise = |run: Length| pitch.rise_over(run);
    let plumb_depth = pitch.slope_length(size.actual_width());
    let inner = -porch.depth + beam_w; // the beam's inside face
    let bottom_at = |y: Length| ledger_top - d_rise(-lt - y) - plumb_depth;
    let beam_top = bottom_at(inner);
    let beam_bottom = beam_top - porch.beam.actual_width();
    if beam_bottom <= post_bottom {
        return Err(fail(format!(
            "at {pitch} the beam would sit below the posts' footing; raise the ledger or shorten the porch"
        )));
    }
    if beam_bottom < HEADROOM {
        warnings.push(format!(
            "the beam is only {beam_bottom} above the floor; raise the ledger or lower the pitch for more headroom"
        ));
    }
    let run = porch.depth + porch.overhang - lt;
    let rafter = RafterDetail {
        name: "porch rafter".to_string(),
        count: 0,
        size,
        length: pitch.slope_length(run),
        pitch,
        plumb_tenths_of_degree: sticks_core::tenths_of_degree(pitch.rise, Pitch::RUN),
        run,
        plumb_depth,
        seat: beam_w,
        heel: d_rise(beam_w),
        heels: vec![pitch.slope_length(porch.depth - lt)],
        top: RafterTop::Ledger,
        upper_overhang: Length::ZERO,
        overhang: porch.overhang,
    };
    if rafter.heel * 3 > size.actual_width() {
        warnings.push(format!(
            "the birdsmouth heel is {:#}, more than a third of the {size} rafter's depth",
            rafter.heel
        ));
    }

    let mut members = Vec::new();
    let mut splices = Vec::new();

    // Posts: one flush with each end, the rest spread evenly between.
    let post_centers = post_centers(width, porch.post, porch.post_spacing);
    let post_x: Vec<Length> = post_centers.iter().map(|&c| c - half(post_w)).collect();
    let post_y = -porch.depth;
    for (i, &x) in post_x.iter().enumerate() {
        let mut p = placed(
            format!("porch-P{}", i + 1),
            MemberKind::Post,
            porch.post,
            Rect {
                x,
                y: post_y,
                w: post_w,
                h: post_d,
            },
            false,
        );
        p.member.length = beam_bottom - post_bottom;
        members.push(p);
    }
    // Without a deck each post has its own pier; with one, the deck's piers
    // under its beam carry the posts.
    let piers = if deck.is_some() {
        Vec::new()
    } else {
        post_centers
            .iter()
            .map(|&x| Pier {
                x,
                y: post_y + half(post_d),
            })
            .collect()
    };

    // The beam, each ply spliced over a post, the plies' joints staggered.
    let beam_longest = longest_board(stock, porch.beam, rules.end_trim);
    let (joints, shared) = ply_joints(width, porch.beam_plies, &post_centers, beam_longest)
        .ok_or_else(|| {
            fail(format!(
                "the beam is longer than the longest {} stock ({beam_longest}) and no post lands where it could be spliced; add posts or longer stock",
                porch.beam
            ))
        })?;
    if shared {
        warnings.push(
            "the beam plies are spliced over the same post; closer posts would let the joints stagger"
                .to_string(),
        );
    }
    for (p, ply_joints) in joints.iter().enumerate() {
        let runs = pieces(Length::ZERO, width, ply_joints);
        let many = runs.len() > 1;
        for (k, (a, b)) in runs.into_iter().enumerate() {
            let mut label = "porch-BM".to_string();
            if porch.beam_plies > 1 {
                label.push((b'a' + p as u8) as char);
            }
            if many {
                label.push_str(&format!("-{}", k + 1));
            }
            members.push(placed(
                label,
                MemberKind::Beam,
                porch.beam,
                Rect {
                    x: a,
                    y: post_y + ply * p as i64,
                    w: b - a,
                    h: ply,
                },
                false,
            ));
        }
        splices.extend(ply_joints.iter().map(|&at| Splice {
            kind: MemberKind::Beam,
            at,
        }));
    }

    // Rafters on the main roof's layout.
    let mut rafter_x = vec![Length::ZERO];
    rafter_x.extend(on_center(width, oc, t, rules.layout_from));
    rafter_x.push(width - t);
    let centers: Vec<Length> = rafter_x.iter().map(|&x| x + half(t)).collect();
    let rafter = RafterDetail {
        count: rafter_x.len(),
        ..rafter
    };
    let plumb = Cut::Plumb {
        tenths_of_degree: rafter.plumb_tenths_of_degree,
    };
    let cuts = vec![
        plumb,
        Cut::Birdsmouth {
            seat: rafter.seat,
            heel: rafter.heel,
            heel_at: rafter.heels[0],
        },
        plumb,
    ];
    let tail = -porch.depth - porch.overhang;
    for (i, &x) in rafter_x.iter().enumerate() {
        let mut p = placed(
            format!("porch-R{:02}", i + 1),
            MemberKind::Rafter,
            size,
            Rect {
                x,
                y: tail,
                w: t,
                h: run,
            },
            true,
        );
        p.member.length = rafter.length;
        p.member.cuts = cuts.clone();
        members.push(p);
    }

    // Ledger and fascia, each spliced over a rafter when too long.
    let fascia = roof.and_then(|r| r.fascia).unwrap_or(two_up(size));
    let ft = fascia.actual_thickness();
    for (prefix, kind, board, y) in [
        ("LG", MemberKind::Ledger, ledger, -lt),
        ("FF", MemberKind::Fascia, fascia, tail - ft),
    ] {
        let longest = longest_board(stock, board, rules.end_trim);
        let joints = split_run(
            Length::ZERO,
            width,
            |s| s <= longest,
            &centers[1..centers.len() - 1],
            &[],
            Length::ZERO,
        )
        .ok_or_else(|| {
            fail(format!(
                "the {} is longer than the longest {board} stock ({longest}) and no rafter lands where it could be spliced; add longer stock",
                kind.name()
            ))
        })?;
        for (n, (a, b)) in pieces(Length::ZERO, width, &joints).into_iter().enumerate() {
            members.push(placed(
                format!("porch-{prefix}{}", n + 1),
                kind,
                board,
                Rect {
                    x: a,
                    y,
                    w: b - a,
                    h: board.actual_thickness(),
                },
                false,
            ));
        }
        splices.extend(joints.iter().map(|&at| Splice { kind, at }));
    }
    if fascia.actual_width() < plumb_depth {
        warnings.push(format!(
            "{fascia} fascia ({:#}) won't cover the rafter tails' {plumb_depth:#} plumb cut",
            fascia.actual_width()
        ));
    }
    splices.sort_by_key(|s| s.at);

    let mut marks: Vec<LayoutMark> = rafter_x
        .iter()
        .enumerate()
        .map(|(i, &x)| LayoutMark {
            label: format!("porch-R{:02}", i + 1),
            kind: MemberKind::Rafter,
            at: match rules.layout_from {
                LayoutStart::Left => x,
                LayoutStart::Right => width - x - t,
            },
        })
        .collect();
    marks.sort_by_key(|m| m.at);

    let top_at = |y: Length| bottom_at(y) + plumb_depth;
    let plane = RoofPlane {
        name: "porch roof".to_string(),
        width,
        eave: (tail, top_at(tail)),
        top: (-lt, ledger_top),
        pitch,
    };
    Ok(FramedPorch {
        width,
        depth: porch.depth,
        heights: PorchHeights {
            ledger_top,
            beam_top,
            beam_bottom,
            post_bottom,
        },
        members,
        rafter,
        piers,
        marks,
        splices,
        plane,
        deck,
        warnings,
    })
}

/// Post centers across `width`: one flush with each end, the rest spread
/// evenly no more than `spacing` apart.
fn post_centers(width: Length, post: Nominal, spacing: Length) -> Vec<Length> {
    let post_w = post.actual_width();
    let span = width - post_w;
    let gaps = ((span.sixteenths() + spacing.sixteenths() - 1) / spacing.sixteenths()).max(1);
    (0..=gaps)
        .map(|k| span.mul_div(k, gaps) + half(post_w))
        .collect()
}

/// The deck: a ledger on the floor's front rim, joists on center out to a
/// rim over the beam, piers under the beam at the posts, and decking boards
/// along the front, a full board at the outer edge and any rip against the
/// shed, end joints on joists and staggered row to row.
fn frame_deck(
    deck: &PorchDeck,
    porch: &Porch,
    building: &Building,
    floor: &Floor,
    rules: &Rules,
    stock: &[Stock],
) -> Result<FramedPorchDeck, FramingError> {
    let fail = |reason: String| FramingError {
        part: "porch deck".to_string(),
        reason,
    };
    let joist = deck.joist.unwrap_or(floor.joist);
    let t = joist.actual_thickness();
    let width = building.width;
    let depth = porch.depth;
    let oc = floor.on_center.unwrap_or(rules.on_center);
    let bw = deck.beam.actual_thickness();
    if deck.step_down < Length::ZERO || deck.gap < Length::ZERO {
        return Err(fail("the step down and gap can't be negative".to_string()));
    }
    if depth <= t * 3 || depth < bw + t {
        return Err(fail(format!("{depth} is too shallow for a deck")));
    }

    let top = -deck.step_down;
    let joist_top = top - deck.decking.actual_thickness();
    let joist_bottom = joist_top - joist.actual_width();
    let beam_bottom = joist_bottom - deck.beam.actual_width();
    let mut warnings = Vec::new();
    let rim_bottom = -(floor.subfloor + floor.joist.actual_width());
    if joist_top - joist.actual_width().mul_div(1, 2) < rim_bottom {
        warnings.push(format!(
            "less than half the deck ledger is against the floor's rim; a smaller step down or {} joists would bolt on better",
            floor.joist
        ));
    }

    let mut members = Vec::new();
    let mut splices = Vec::new();
    let (joist_x, centers) = joist_layout(width, oc, t, rules.layout_from);

    // Ledger and outer rim along the front, spliced over a joist.
    let joints = joints_over_joists("rim", joist, width, &centers, stock, rules).map_err(fail)?;
    for (prefix, kind, y) in [
        ("porch-DL", MemberKind::Ledger, -t),
        ("porch-DR", MemberKind::RimJoist, -depth),
    ] {
        members.extend(run_pieces(prefix, 1, kind, joist, width, y, t, &joints));
        splices.extend(joints.iter().map(|&at| Splice { kind, at }));
    }
    for (i, &x) in joist_x.iter().enumerate() {
        members.push(placed(
            format!("porch-DJ{:02}", i + 1),
            MemberKind::Joist,
            joist,
            Rect {
                x,
                y: -depth + t,
                w: t,
                h: depth - t * 2,
            },
            true,
        ));
    }

    // The beam under the outer rim, on a pier under each post.
    let pier_x = post_centers(width, porch.post, porch.post_spacing);
    let (beam_joints, _) = beam_joints(deck.beam, 1, width, &pier_x, "pier", stock, rules)
        .map_err(|r| fail(r.replace("add piers", "add posts")))?;
    let (beam, beam_splices) = beam_line(
        "porch-DB",
        MemberKind::Beam,
        deck.beam,
        width,
        -depth,
        &beam_joints,
    );
    members.extend(beam);
    splices.extend(beam_splices);
    let piers = pier_x
        .iter()
        .map(|&x| Pier {
            x,
            y: -depth + half(bw),
        })
        .collect();

    // Decking rows from the outer edge in, joints on joists, staggered.
    let laid = Decking {
        prefix: "porch-DK",
        width,
        y0: -depth,
        y1: Length::ZERO,
        centers: &centers,
        board: deck.decking,
        gap: deck.gap,
        oc,
        rip_at: "against the shed",
    }
    .lay(stock, rules)
    .map_err(fail)?;
    members.extend(laid.members);
    warnings.extend(laid.warnings);
    let rows = laid.rows;

    let marks = joist_marks("porch-DJ", &joist_x, t, width, rules.layout_from);
    splices.sort_by_key(|s| s.at);

    Ok(FramedPorchDeck {
        heights: PorchDeckHeights {
            top,
            joist_top,
            joist_bottom,
            beam_bottom,
        },
        members,
        piers,
        marks,
        splices,
        rows,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sticks_core::{MemberKind, Project};

    fn l(s: &str) -> Length {
        s.parse().unwrap()
    }

    fn porch(extra: &str) -> Result<FramedPorch, FramingError> {
        let p = Project::from_toml(&format!(
            r#"
name = "Shed"
[building]
width = "18'"
depth = "14'"
height = "10'"
[roof]
pitch = "2:12"
[porch]
{extra}
"#
        ))
        .unwrap();
        frame_porch(
            p.porch.as_ref().unwrap(),
            p.building.as_ref().unwrap(),
            p.roof.as_ref(),
            p.floor.as_ref(),
            &p.rules,
            &p.stock,
        )
    }

    /// Worked by hand: the ledger top at 117" (under two 1-1/2" plates), a
    /// 2x6 rafter's top edge dropping 2" a foot from the ledger's face.
    #[test]
    fn porch_matches_a_hand_calculation() {
        let f = porch("").unwrap();
        assert_eq!(f.heights.ledger_top, l("117"));
        // 72 - 1-1/2 - 3 = 67-1/2" of run from the ledger face to the beam's
        // inside face drops 11-1/4"; 2x6 at 2:12 is 5-9/16" plumb.
        let r = &f.rafter;
        assert_eq!(r.plumb_depth, l("5-9/16"));
        assert_eq!(f.heights.beam_top, l("117") - l("11-1/4") - l("5-9/16"));
        assert_eq!(f.heights.beam_bottom, f.heights.beam_top - l("7-1/4"));
        assert_eq!(r.seat, l("3"));
        assert_eq!(r.heel, l("1/2"));
        // 82-1/2" of run, ledger face to tail.
        assert_eq!(r.run, l("82-1/2"));
        assert_eq!(r.length, l("83-5/8"));
        assert_eq!(r.count, 15);

        let posts: Vec<_> = f
            .members
            .iter()
            .filter(|p| p.member.kind == MemberKind::Post)
            .collect();
        // 18' at 8' or less between posts: three gaps, four posts.
        assert_eq!(posts.len(), 4);
        assert_eq!(f.piers.len(), 4);
        assert!(posts
            .iter()
            .all(|p| p.member.length == f.heights.beam_bottom));
        // Two 2x8 plies, each spliced over a different post.
        let beam_joints: Vec<_> = f
            .splices
            .iter()
            .filter(|s| s.kind == MemberKind::Beam)
            .map(|s| s.at)
            .collect();
        assert_eq!(beam_joints.len(), 2);
        assert_ne!(beam_joints[0], beam_joints[1]);
        assert!(f.warnings.is_empty(), "{:?}", f.warnings);
    }

    /// A 6' deck 1" below a 3/4" subfloor on 2x6 joists: 5/4x6 decking
    /// with 1/8" gaps takes 13 rows, the last ripped against the shed.
    #[test]
    fn deck_matches_a_hand_calculation() {
        let f = porch("[floor]\n[porch.deck]").unwrap();
        let d = f.deck.as_ref().unwrap();
        assert_eq!(d.heights.top, l("-1"));
        assert_eq!(d.heights.joist_top, l("-2"));
        assert_eq!(d.heights.joist_bottom, l("-7-1/2"));
        assert_eq!(d.heights.beam_bottom, l("-13"));
        // The posts stand on the deck, so they're shorter.
        assert_eq!(f.heights.post_bottom, l("-1"));
        assert!(f.piers.is_empty());
        assert_eq!(d.piers.len(), 4);
        // 72 / (5-1/2 + 1/8) = 12.8 rows: 12 whole boards and a rip.
        assert_eq!(d.rows, 13);
        assert!(
            d.warnings[0].contains("ripped to 4-1/2\""),
            "{:?}",
            d.warnings
        );
        let joists: Vec<_> = d
            .members
            .iter()
            .filter(|p| p.member.kind == MemberKind::Joist)
            .collect();
        assert_eq!(joists.len(), 15);
        assert!(joists.iter().all(|j| j.member.length == l("69")));
        // 18' decking from 16' stock: rows splice on alternating joists.
        let joints: Vec<_> = d
            .members
            .iter()
            .filter(|p| p.member.kind == MemberKind::Decking && p.rect.x > Length::ZERO)
            .map(|p| p.rect.x)
            .take(2)
            .collect();
        assert_ne!(joints[0], joints[1]);
    }

    #[test]
    fn a_deck_needs_a_floor() {
        let err = porch("[porch.deck]").unwrap_err();
        assert!(err.reason.contains("floor"), "{err}");
    }

    #[test]
    fn a_low_beam_warns_about_headroom() {
        let f = porch("depth = \"16'\"\npitch = \"4:12\"\nledger_height = \"8'\"").unwrap();
        assert!(
            f.warnings.iter().any(|w| w.contains("headroom")),
            "{:?}",
            f.warnings
        );
    }

    #[test]
    fn a_porch_needs_a_pitch_without_a_roof() {
        let p = Project::from_toml(
            "name = \"x\"\n[building]\nwidth = \"10'\"\ndepth = \"8'\"\nheight = \"8'\"\n[porch]\n",
        )
        .unwrap();
        let err = frame_porch(
            p.porch.as_ref().unwrap(),
            p.building.as_ref().unwrap(),
            None,
            None,
            &p.rules,
            &p.stock,
        )
        .unwrap_err();
        assert!(err.reason.contains("pitch"), "{err}");
    }
}
