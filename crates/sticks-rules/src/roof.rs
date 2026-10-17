//! Roofs over the building: a lean-to, or a gable with a ridge board.
//!
//! A lean-to has rafters on center from the front wall to the back, each with
//! a birdsmouth over both walls and plumb-cut tails. A gable has a pair of
//! rafters at each spot, one from each eave wall up to a ridge board that
//! runs along the front, with collar ties across some pairs. Fascia runs
//! across the tails either way.
//!
//! Plan coordinates match the floor: `x` along the front from the left end,
//! `y` from the outside face of the front wall toward the back. The front
//! overhang sits at negative `y`.
//!
//! Section geometry, front to back: each birdsmouth's seat is the full depth
//! of the wall. Under a lean-to the rafter's bottom edge rests on the front
//! wall's cap plate at its outside face and on the back wall's at its inside
//! face, and the side walls are rake walls whose tops meet the rafters. Under
//! a gable it rests on each eave wall's cap plate at the inside face, and the
//! side walls are level with gable studs above them.

use serde::Serialize;
use sticks_core::{
    longest_board, tenths_of_degree, Building, Cut, LayoutStart, Length, MemberKind, Nominal,
    Pitch, Roof, RoofKind, Rules, Stock,
};

use crate::layout::{
    half, on_center, overlaps, pieces, placed, split_run, LayoutMark, Placed, Rect, Splice,
};
use crate::FramingError;

/// Collar ties are spaced no more than this far apart.
const COLLAR_TIE_SPACING: Length = Length::inches(48);
/// Gable studs shorter than this are left out; the rafter is close enough.
const SHORT_GABLE_STUD: Length = Length::inches(3);

/// Heights the walls under a roof need, subfloor to top of cap plate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct WallHeights {
    pub front: Length,
    pub back: Length,
    /// Where the side walls meet the inside face of the front wall.
    pub side_front: Length,
    /// Where the side walls meet the inside face of the back wall.
    pub side_back: Length,
}

/// What a rafter's upper end meets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RafterTop {
    /// Nothing: the upper end is a tail past the high wall (a lean-to).
    Tail,
    /// It butts the ridge board.
    Ridge,
    /// It hangs on a ledger.
    Ledger,
}

/// How to cut one set of rafters that are all alike.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RafterDetail {
    /// `rafter`, `front rafter`, `back rafter` or `porch rafter`.
    pub name: String,
    pub count: usize,
    pub size: Nominal,
    /// Plumb cut to plumb cut, along either edge.
    pub length: Length,
    pub pitch: Pitch,
    /// Plumb cuts and the heel cuts are this far from square.
    pub plumb_tenths_of_degree: i32,
    /// Level run from end to end.
    pub run: Length,
    /// The rafter's depth measured plumb.
    pub plumb_depth: Length,
    /// Level seat cut, the full depth of what it sits on.
    pub seat: Length,
    /// Plumb heel cut.
    pub heel: Length,
    /// Along the bottom edge from the upper end to each birdsmouth's heel.
    pub heels: Vec<Length>,
    pub top: RafterTop,
    /// Level overhang past the upper wall, for a lean-to; zero otherwise.
    pub upper_overhang: Length,
    /// Level overhang past the lower support.
    pub overhang: Length,
}

/// Where a gable roof's ridge board sits, in section.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Ridge {
    /// The ridge board's front face.
    pub y: Length,
    pub thickness: Length,
    pub top: Length,
    pub bottom: Length,
}

/// One flat face of a roof, along the top edges of its rafters, from the
/// tails up. It spans `x` from 0 to `width`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RoofPlane {
    /// `roof`, `front slope`, `back slope` or `porch roof`.
    pub name: String,
    pub width: Length,
    /// Plan `y` and height of the top edge at the tails.
    pub eave: (Length, Length),
    /// Plan `y` and height of the top edge at the upper end.
    pub top: (Length, Length),
    pub pitch: Pitch,
}

impl RoofPlane {
    /// Length up the slope from the tails to the upper end.
    pub fn slope_length(&self) -> Length {
        self.pitch.slope_length((self.top.0 - self.eave.0).abs())
    }
}

/// Everything generated for the roof.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FramedRoof {
    pub kind: RoofKind,
    pub width: Length,
    pub depth: Length,
    pub heights: WallHeights,
    /// Rafters, then the ridge and collar ties, then fascia, in plan.
    pub members: Vec<Placed>,
    /// One per set of rafters cut alike.
    pub rafters: Vec<RafterDetail>,
    /// Rafter spots, ordered from the layout end, as marked on the cap plates.
    pub marks: Vec<LayoutMark>,
    pub splices: Vec<Splice>,
    pub ridge: Option<Ridge>,
    pub planes: Vec<RoofPlane>,
    pub warnings: Vec<String>,
}

impl FramedRoof {
    /// Height of the rafters' bottom edge (or the ridge board's, under it)
    /// over plan `y`, for a gable roof. Used to fit the gable studs.
    pub fn underside(&self, y: Length) -> Length {
        let Some(ridge) = self.ridge else {
            return self.heights.front;
        };
        let pitch = self.rafters[0].pitch;
        let wall_depth = self.rafters[0].seat;
        if y < ridge.y {
            self.heights.front + pitch.rise_over(y - wall_depth)
        } else if y > ridge.y + ridge.thickness {
            self.heights.back + pitch.rise_over(self.depth - wall_depth - y)
        } else {
            ridge.bottom
        }
    }
}

/// The wall heights that put every rafter's bottom edge on the cap plates.
/// `wall_depth` is the actual width of the wall studs.
pub fn wall_heights(building: &Building, roof: &Roof, wall_depth: Length) -> WallHeights {
    let front = building.height;
    match roof.kind {
        RoofKind::LeanTo => {
            let back = front - roof.pitch.rise_over(building.depth - wall_depth);
            WallHeights {
                front,
                back,
                side_front: front - roof.pitch.rise_over(wall_depth),
                side_back: back,
            }
        }
        RoofKind::Gable => WallHeights {
            front,
            back: front,
            side_front: front,
            side_back: front,
        },
    }
}

/// The two sizes up from `size` that fascia and ridge boards default to.
pub fn two_up(size: Nominal) -> Nominal {
    size.with_width(size.width.saturating_add(2))
}

pub fn frame_roof(
    roof: &Roof,
    building: &Building,
    rules: &Rules,
    stock: &[Stock],
) -> Result<FramedRoof, FramingError> {
    let fail = |reason: String| FramingError {
        part: "roof".to_string(),
        reason,
    };
    let size = roof.rafter;
    let t = size.actual_thickness();
    let wall_depth = rules.wall_lumber.actual_width();
    let (width, depth) = (building.width, building.depth);
    let oc = roof.on_center.unwrap_or(rules.on_center);
    if oc < t * 2 {
        return Err(fail(format!(
            "rafters {oc:#} on center leaves no room between rafters"
        )));
    }
    if roof.front_overhang < Length::ZERO || roof.back_overhang < Length::ZERO {
        return Err(fail("overhangs can't be negative".to_string()));
    }
    if depth <= wall_depth * 2 || width < t * 2 {
        return Err(fail(format!("{width} by {depth} is too small to roof")));
    }
    let mut rafter_x = vec![Length::ZERO];
    rafter_x.extend(on_center(width, oc, t, rules.layout_from));
    rafter_x.push(width - t);
    let centers: Vec<Length> = rafter_x.iter().map(|&x| x + half(t)).collect();

    let mut framed = match roof.kind {
        RoofKind::LeanTo => lean_to(roof, building, wall_depth, &rafter_x),
        RoofKind::Gable => gable(roof, building, rules, stock, wall_depth, &rafter_x)?,
    };
    if let Some(d) = framed.rafters.first() {
        if d.heel * 3 > size.actual_width() {
            framed.warnings.push(format!(
                "the birdsmouth heel is {:#}, more than a third of the {size} rafter's depth",
                d.heel
            ));
        }
    }

    // Fascia across the tails, spliced over a rafter when too long.
    let fascia = roof.fascia.unwrap_or(two_up(size));
    let plumb_depth = framed.rafters[0].plumb_depth;
    if fascia.actual_width() < plumb_depth {
        framed.warnings.push(format!(
            "{fascia} fascia ({:#}) won't cover the rafter tails' {:#} plumb cut",
            fascia.actual_width(),
            plumb_depth
        ));
    }
    let ft = fascia.actual_thickness();
    let joints = run_joints(width, &centers, longest_board(stock, fascia, rules.end_trim))
        .ok_or_else(|| {
            fail(format!(
                "the fascia is longer than the longest {fascia} stock ({}) and no rafter lands where it could be spliced; add longer stock",
                longest_board(stock, fascia, rules.end_trim)
            ))
        })?;
    for (prefix, y) in [
        ("FF", -roof.front_overhang - ft),
        ("BF", depth + roof.back_overhang),
    ] {
        for (n, (a, b)) in pieces(Length::ZERO, width, &joints).into_iter().enumerate() {
            framed.members.push(placed(
                format!("roof-{prefix}{}", n + 1),
                MemberKind::Fascia,
                fascia,
                Rect {
                    x: a,
                    y,
                    w: b - a,
                    h: ft,
                },
                false,
            ));
        }
        framed.splices.extend(joints.iter().map(|&at| Splice {
            kind: MemberKind::Fascia,
            at,
        }));
    }

    framed.marks = rafter_x
        .iter()
        .enumerate()
        .map(|(i, &x)| LayoutMark {
            label: match roof.kind {
                RoofKind::LeanTo => format!("roof-R{:02}", i + 1),
                RoofKind::Gable => format!("roof-R{:02} pair", i + 1),
            },
            kind: MemberKind::Rafter,
            at: match rules.layout_from {
                LayoutStart::Left => x,
                LayoutStart::Right => width - x - t,
            },
        })
        .collect();
    framed.marks.sort_by_key(|m| m.at);
    Ok(framed)
}

/// Joints that split a run from 0 to `width` over `centers` into pieces no
/// longer than `longest`.
fn run_joints(width: Length, centers: &[Length], longest: Length) -> Option<Vec<Length>> {
    let inner = &centers[1.min(centers.len())..centers.len().saturating_sub(1)];
    split_run(
        Length::ZERO,
        width,
        |s| s <= longest,
        inner,
        &[],
        Length::ZERO,
    )
}

fn rafter(name: &str, count: usize, roof: &Roof, seat: Length, run: Length) -> RafterDetail {
    let pitch = roof.pitch;
    RafterDetail {
        name: name.to_string(),
        count,
        size: roof.rafter,
        length: pitch.slope_length(run),
        pitch,
        plumb_tenths_of_degree: tenths_of_degree(pitch.rise, Pitch::RUN),
        run,
        plumb_depth: pitch.slope_length(roof.rafter.actual_width()),
        seat,
        heel: pitch.rise_over(seat),
        heels: Vec::new(),
        top: RafterTop::Tail,
        upper_overhang: Length::ZERO,
        overhang: Length::ZERO,
    }
}

fn birdsmouths(d: &RafterDetail) -> Vec<Cut> {
    let plumb = Cut::Plumb {
        tenths_of_degree: d.plumb_tenths_of_degree,
    };
    let mut cuts = vec![plumb];
    cuts.extend(d.heels.iter().map(|&heel_at| Cut::Birdsmouth {
        seat: d.seat,
        heel: d.heel,
        heel_at,
    }));
    cuts.push(plumb);
    cuts
}

fn lean_to(
    roof: &Roof,
    building: &Building,
    wall_depth: Length,
    rafter_x: &[Length],
) -> FramedRoof {
    let (width, depth) = (building.width, building.depth);
    let pitch = roof.pitch;
    let heights = wall_heights(building, roof, wall_depth);
    let run = roof.front_overhang + depth + roof.back_overhang;
    let mut d = rafter("rafter", rafter_x.len(), roof, wall_depth, run);
    d.heels = vec![
        pitch.slope_length(roof.front_overhang + wall_depth),
        pitch.slope_length(roof.front_overhang + depth),
    ];
    d.upper_overhang = roof.front_overhang;
    d.overhang = roof.back_overhang;
    let cuts = birdsmouths(&d);
    let t = roof.rafter.actual_thickness();
    let members = rafter_x
        .iter()
        .enumerate()
        .map(|(i, &x)| {
            let mut p = placed(
                format!("roof-R{:02}", i + 1),
                MemberKind::Rafter,
                roof.rafter,
                Rect {
                    x,
                    y: -roof.front_overhang,
                    w: t,
                    h: run,
                },
                true,
            );
            p.member.length = d.length;
            p.member.cuts = cuts.clone();
            p
        })
        .collect();
    // The bottom edge is at the front wall's height over its outside face.
    let top_at = |y: Length| heights.front - pitch.rise_over(y) + d.plumb_depth;
    let plane = RoofPlane {
        name: "roof".to_string(),
        width,
        eave: (
            depth + roof.back_overhang,
            top_at(depth + roof.back_overhang),
        ),
        top: (-roof.front_overhang, top_at(-roof.front_overhang)),
        pitch,
    };
    FramedRoof {
        kind: RoofKind::LeanTo,
        width,
        depth,
        heights,
        members,
        rafters: vec![d],
        marks: Vec::new(),
        splices: Vec::new(),
        ridge: None,
        planes: vec![plane],
        warnings: Vec::new(),
    }
}

fn gable(
    roof: &Roof,
    building: &Building,
    rules: &Rules,
    stock: &[Stock],
    wall_depth: Length,
    rafter_x: &[Length],
) -> Result<FramedRoof, FramingError> {
    let fail = |reason: String| FramingError {
        part: "roof".to_string(),
        reason,
    };
    let (width, depth) = (building.width, building.depth);
    let pitch = roof.pitch;
    let heights = wall_heights(building, roof, wall_depth);
    let eave = heights.front;
    let t = roof.rafter.actual_thickness();
    let ridge_size = roof.ridge.unwrap_or(two_up(roof.rafter));
    let rt = ridge_size.actual_thickness();
    // Each rafter's plumb cut meets a face of the ridge board.
    let ridge_y = half(depth) - half(rt);
    let reach = ridge_y; // level from the outside face of an eave wall to the ridge
    let n = rafter_x.len();
    let same = roof.front_overhang == roof.back_overhang;
    let mut details = Vec::new();
    for (name, overhang) in [
        ("front rafter", roof.front_overhang),
        ("back rafter", roof.back_overhang),
    ] {
        let name = if same { "rafter" } else { name };
        let mut d = rafter(
            name,
            if same { n * 2 } else { n },
            roof,
            wall_depth,
            overhang + reach,
        );
        d.heels = vec![pitch.slope_length(reach)];
        d.top = RafterTop::Ridge;
        d.overhang = overhang;
        details.push(d);
        if same {
            break;
        }
    }
    let front = &details[0];
    let back = details.last().unwrap_or(front);

    // Bottom edge over the front slope: on the cap plate at the inside face.
    let bottom = |y: Length| eave + pitch.rise_over(y - wall_depth);
    let top = bottom(ridge_y) + front.plumb_depth;
    let ridge = Ridge {
        y: ridge_y,
        thickness: rt,
        top,
        bottom: top - ridge_size.actual_width(),
    };
    let mut warnings = Vec::new();
    if ridge.bottom > bottom(ridge_y) {
        warnings.push(format!(
            "the {ridge_size} ridge board is shallower than the rafters' {:#} plumb cut",
            front.plumb_depth
        ));
    }

    let mut members = Vec::new();
    for (i, &x) in rafter_x.iter().enumerate() {
        for (side, d, y) in [
            ("F", front, -roof.front_overhang),
            ("B", back, depth - ridge_y),
        ] {
            let mut p = placed(
                format!("roof-R{:02}{side}", i + 1),
                MemberKind::Rafter,
                roof.rafter,
                Rect {
                    x,
                    y,
                    w: t,
                    h: d.run,
                },
                true,
            );
            p.member.length = d.length;
            p.member.cuts = birdsmouths(d);
            members.push(p);
        }
    }

    // The ridge board, spliced over a rafter pair when too long.
    let centers: Vec<Length> = rafter_x.iter().map(|&x| x + half(t)).collect();
    let longest = longest_board(stock, ridge_size, rules.end_trim);
    let joints = run_joints(width, &centers, longest).ok_or_else(|| {
        fail(format!(
            "the ridge board is longer than the longest {ridge_size} stock ({longest}) and no rafter lands where it could be spliced; add longer stock"
        ))
    })?;
    for (k, (a, b)) in pieces(Length::ZERO, width, &joints).into_iter().enumerate() {
        members.push(placed(
            format!("roof-RB{}", k + 1),
            MemberKind::Ridge,
            ridge_size,
            Rect {
                x: a,
                y: ridge_y,
                w: b - a,
                h: rt,
            },
            false,
        ));
    }
    let mut splices: Vec<Splice> = joints
        .iter()
        .map(|&at| Splice {
            kind: MemberKind::Ridge,
            at,
        })
        .collect();

    // Collar ties in the upper third, on interior pairs no more than 4'
    // apart, square cut with their top corners on the rafters' top edges.
    let tie = roof.collar_tie.unwrap_or(Nominal::new(2, 4));
    let oc = roof.on_center.unwrap_or(rules.on_center);
    let every = (COLLAR_TIE_SPACING.sixteenths() / oc.sixteenths().max(1)).max(1) as usize;
    let tie_y = ridge_y - (ridge_y - wall_depth).mul_div(1, 3);
    let tie_len = depth - tie_y * 2;
    let mut ties = 0;
    if tie_len > Length::ZERO {
        for (i, &x) in rafter_x
            .iter()
            .enumerate()
            .skip(1)
            .take(n.saturating_sub(2))
        {
            if i % every != 0 {
                continue;
            }
            ties += 1;
            // Beside the rafter, on the side away from the nearer end.
            let tx = if x + t + tie.actual_thickness() <= width - t {
                x + t
            } else {
                x - tie.actual_thickness()
            };
            members.push(placed(
                format!("roof-CT{ties:02}"),
                MemberKind::CollarTie,
                tie,
                Rect {
                    x: tx,
                    y: tie_y,
                    w: tie.actual_thickness(),
                    h: tie_len,
                },
                true,
            ));
        }
    }
    if ties == 0 && n > 2 {
        warnings.push("no rafter pair has room for a collar tie".to_string());
    }
    splices.sort_by_key(|s| s.at);

    let top_at = |y: Length| bottom(y) + front.plumb_depth;
    let mid = half(depth);
    let back_top = |y: Length| eave + pitch.rise_over(depth - wall_depth - y) + back.plumb_depth;
    let planes = vec![
        RoofPlane {
            name: "front slope".to_string(),
            width,
            eave: (-roof.front_overhang, top_at(-roof.front_overhang)),
            top: (mid, top_at(mid)),
            pitch,
        },
        RoofPlane {
            name: "back slope".to_string(),
            width,
            eave: (
                depth + roof.back_overhang,
                back_top(depth + roof.back_overhang),
            ),
            top: (mid, back_top(mid)),
            pitch,
        },
    ];
    Ok(FramedRoof {
        kind: RoofKind::Gable,
        width,
        depth,
        heights,
        members,
        rafters: details,
        marks: Vec::new(),
        splices,
        ridge: Some(ridge),
        planes,
        warnings,
    })
}

/// Studs that fill a gable end between a side wall's cap plate and the end
/// rafters above it, one over each of the wall's layout studs and one under
/// the ridge. `plan_y` turns a spot along the wall (from its left end, seen
/// from outside) into plan `y`. Returned in the wall's elevation, labeled
/// for the wall.
pub fn gable_studs(
    roof: &FramedRoof,
    wall_id: &str,
    wall_top: Length,
    stud_x: &[Length],
    lumber: Nominal,
    plan_y: impl Fn(Length) -> Length,
) -> Vec<Placed> {
    let Some(ridge) = roof.ridge else {
        return Vec::new();
    };
    let t = lumber.actual_thickness();
    let (y0, y1) = (plan_y(Length::ZERO), plan_y(Length::inches(1)));
    // Where the ridge board crosses the wall, in the wall's own `x`.
    let wall_x = |y: Length| if y1 > y0 { y - y0 } else { y0 - y };
    let (ra, rb) = {
        let (a, b) = (wall_x(ridge.y), wall_x(ridge.y + ridge.thickness));
        (a.min(b), a.max(b))
    };
    let mut xs: Vec<Length> = stud_x
        .iter()
        .copied()
        .filter(|&x| !overlaps((x, x + t), (ra, rb)))
        .collect();
    let under = ra + half(rb - ra) - half(t);
    xs.retain(|&x| !overlaps((x, x + t), (under, under + t)));
    xs.push(under);
    xs.sort();

    let angle = tenths_of_degree(roof.rafters[0].pitch.rise, Pitch::RUN);
    let mut out = Vec::new();
    for x in xs {
        let flat = overlaps((x, x + t), (ra, rb));
        let (ha, hb) = if flat {
            (ridge.bottom, ridge.bottom)
        } else {
            (roof.underside(plan_y(x)), roof.underside(plan_y(x + t)))
        };
        let long = ha.max(hb);
        if long - wall_top < SHORT_GABLE_STUD {
            continue;
        }
        let mut p = placed(
            format!("{wall_id}-G{:02}", out.len() + 1),
            MemberKind::GableStud,
            lumber,
            Rect {
                x,
                y: wall_top,
                w: t,
                h: long - wall_top,
            },
            true,
        );
        if !flat {
            p.member.cuts = vec![Cut::Angled {
                tenths_of_degree: angle,
            }];
            p.outline = vec![(x, wall_top), (x + t, wall_top), (x + t, hb), (x, ha)];
        }
        out.push(p);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use sticks_core::Project;

    fn l(s: &str) -> Length {
        s.parse().unwrap()
    }

    const SHED: &str = r#"
name = "Shed"
[building]
width = "18'"
depth = "14'"
height = "10'"
[roof]
pitch = "3:12"
[[stock]]
size = "2x6"
lengths = ["8'", "10'", "12'", "14'", "16'", "18'"]
[[stock]]
size = "2x8"
lengths = ["8'", "16'"]
"#;

    fn shed() -> FramedRoof {
        let p = Project::from_toml(SHED).unwrap();
        frame_roof(
            p.roof.as_ref().unwrap(),
            p.building.as_ref().unwrap(),
            &p.rules,
            &p.stock,
        )
        .unwrap()
    }

    /// Worked by hand: 3:12 is 14.04°, and a level foot runs 12.369" along
    /// the slope.
    #[test]
    fn rafters_match_a_hand_calculation() {
        let r = shed();
        // The back wall sits 164-1/2" of run behind the front wall's seat.
        assert_eq!(r.heights.back, l("120") - l("41-1/8"));
        assert_eq!(r.heights.side_front, l("120") - l("7/8"));
        assert_eq!(r.heights.side_back, r.heights.back);

        let d = &r.rafters[0];
        assert_eq!(d.count, 15);
        assert_eq!(d.run, l("192"));
        // 192 * 1.030776 = 197.909, nearest 197-15/16.
        assert_eq!(d.length, l("197-15/16"));
        assert_eq!(d.plumb_tenths_of_degree, 140);
        assert_eq!(d.heel, l("7/8"));
        assert_eq!(d.seat, l("3-1/2"));
        // 15-1/2" and 180" of run to the heels.
        assert_eq!(d.heels, [l("16"), l("185-9/16")]);
        assert_eq!(d.plumb_depth, l("5-11/16"));

        let rafters: Vec<_> = r
            .members
            .iter()
            .filter(|p| p.member.kind == MemberKind::Rafter)
            .collect();
        assert_eq!(rafters.len(), 15);
        assert_eq!(rafters[0].member.cuts.len(), 4);
        assert_eq!(r.marks[1].at, l("15-1/4"));

        // 2x8 fascia, spliced at the rafter centered on 16'.
        let fascia: Vec<_> = r
            .members
            .iter()
            .filter(|p| p.member.kind == MemberKind::Fascia)
            .map(|p| (p.member.label.as_str(), p.member.length))
            .collect();
        assert_eq!(
            fascia,
            [
                ("roof-FF1", l("16'")),
                ("roof-FF2", l("24")),
                ("roof-BF1", l("16'")),
                ("roof-BF2", l("24")),
            ]
        );
        assert!(r.warnings.is_empty(), "{:?}", r.warnings);
    }

    #[test]
    fn steep_pitches_warn_about_deep_heels() {
        let p = Project::from_toml(&SHED.replace("3:12", "8:12")).unwrap();
        let r = frame_roof(
            p.roof.as_ref().unwrap(),
            p.building.as_ref().unwrap(),
            &p.rules,
            &p.stock,
        )
        .unwrap();
        assert_eq!(r.rafters[0].heel, l("2-5/16"));
        assert!(
            r.warnings[0].contains("more than a third"),
            "{:?}",
            r.warnings
        );
    }
}
