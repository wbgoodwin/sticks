//! Floor platforms: rim joists, joists on center, blocking over the inner
//! beams, beams on piers or skids, and the subfloor sheet layout.
//!
//! Coordinates are a plan seen from above: `x` runs along the front of the
//! building from its left end, `y` from the front edge toward the back.
//! Joists run front to back; rims and beams run along the front.

use serde::Serialize;
use sticks_core::{
    Building, Floor, FloorSupport, Length, MemberKind, Rules, SheetPiece, SheetStock, SheetUse,
    Stock,
};

use crate::layout::{half, placed, sheet_rows, LayoutMark, Placed, Rect, Splice};
use crate::platform::{
    beam_joints, beam_line, joints_over_joists, joist_layout, joist_marks, run_pieces, support_line,
};
use crate::FramingError;

/// Clear space between two joists, where foam board or insulation goes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Bay {
    /// From the left end of the floor to the bay's left side.
    pub x: Length,
    pub width: Length,
}

/// A piece of subfloor and where it goes, in plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PlacedSheet {
    pub piece: SheetPiece,
    pub rect: Rect,
}

/// A pier under a beam, by its center.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Pier {
    pub x: Length,
    pub y: Length,
}

/// Everything generated for the floor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FramedFloor {
    pub width: Length,
    pub depth: Length,
    /// Rims, joists and blocking, then the beams under them, then any posts
    /// under the beams.
    pub members: Vec<Placed>,
    pub piers: Vec<Pier>,
    /// Bays between joists, left to right.
    pub bays: Vec<Bay>,
    /// Joists, ordered from the layout end, as marked on the rims.
    pub marks: Vec<LayoutMark>,
    pub subfloor: Vec<PlacedSheet>,
    pub splices: Vec<Splice>,
    pub warnings: Vec<String>,
}

pub fn frame_floor(
    floor: &Floor,
    building: &Building,
    rules: &Rules,
    stock: &[Stock],
    sheets: &[SheetStock],
) -> Result<FramedFloor, FramingError> {
    let fail = |reason: String| FramingError {
        part: "floor".to_string(),
        reason,
    };
    let joist = floor.joist;
    let t = joist.actual_thickness();
    let (width, depth) = (building.width, building.depth);
    let oc = floor.on_center.unwrap_or(rules.on_center);
    if oc < t * 2 {
        return Err(fail(format!(
            "joists {oc:#} on center leaves no room between joists"
        )));
    }
    if width < t * 4 || depth < t * 4 {
        return Err(fail(format!("{width} by {depth} is too small to frame")));
    }
    if floor.beams < 2 {
        return Err(fail(
            "needs at least two beams, one under each rim".to_string(),
        ));
    }
    if floor.beam_plies == 0 {
        return Err(fail("a beam needs at least one ply".to_string()));
    }
    let ply = floor.beam.actual_thickness();
    let beam_width = ply * i64::from(floor.beam_plies);
    if beam_width * i64::from(floor.beams) > depth {
        return Err(fail(format!(
            "{} beams {beam_width:#} wide don't fit under a {depth} floor",
            floor.beams
        )));
    }
    let piers_used = floor.support != FloorSupport::Skids;
    if floor.support == FloorSupport::Posts && floor.post_length() <= Length::ZERO {
        return Err(fail("posts under the beams need a height".to_string()));
    }
    if piers_used && floor.pier_spacing <= Length::ZERO {
        return Err(fail("pier spacing must be more than zero".to_string()));
    }
    if piers_used && (floor.pier_inset < Length::ZERO || floor.pier_inset * 2 >= width) {
        return Err(fail(format!(
            "piers set in {} from each end leave no room along a {width} beam",
            floor.pier_inset
        )));
    }

    let mut members = Vec::new();
    let mut splices = Vec::new();
    let mut warnings = Vec::new();

    // Joists: one flush with each end, the rest on layout.
    let (joist_x, centers) = joist_layout(width, oc, t, rules.layout_from);

    // Rims along the front and back, spliced over a joist when too long.
    let rim_joints = joints_over_joists("rim joists", joist, width, &centers, stock, rules)
        .map_err(|r| {
            fail(
                r.replace("the rim joists is longer", "the rim joists are longer")
                    .replace("where it could", "where they could"),
            )
        })?;
    let mut rim_count = 0;
    for y in [Length::ZERO, depth - t] {
        let rims = run_pieces(
            "floor-RJ",
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
            format!("floor-J{:02}", i + 1),
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
    let bays: Vec<Bay> = joist_x
        .windows(2)
        .map(|p| Bay {
            x: p[0] + t,
            width: p[1] - p[0] - t,
        })
        .collect();

    // Beam lines: the outer two under the rims, the rest evenly between.
    let n = i64::from(floor.beams);
    let beam_y: Vec<Length> = (0..n)
        .map(|i| (depth - beam_width).mul_div(i, n - 1))
        .collect();

    // Solid blocking between joists over each inner beam.
    let mut block_count = 0;
    for &y in &beam_y[1..beam_y.len() - 1] {
        let row = y + half(beam_width) - half(t);
        for bay in &bays {
            block_count += 1;
            members.push(placed(
                format!("floor-B{block_count:02}"),
                MemberKind::Blocking,
                joist,
                Rect {
                    x: bay.x,
                    y: row,
                    w: bay.width,
                    h: t,
                },
                false,
            ));
        }
    }

    // Piers along each beam, evenly spaced from one inset end to the other.
    let pier_x: Vec<Length> = if piers_used {
        support_line(width, floor.pier_inset, floor.pier_spacing)
    } else {
        Vec::new()
    };
    let piers: Vec<Pier> = beam_y
        .iter()
        .flat_map(|&y| {
            pier_x.iter().map(move |&x| Pier {
                x,
                y: y + half(beam_width),
            })
        })
        .collect();

    // Beams: each ply spliced over a pier (or a joist, for skids), the plies'
    // joints kept on different supports.
    let (kind, prefix) = match floor.support {
        FloorSupport::Piers | FloorSupport::Posts => (MemberKind::Beam, "BM"),
        FloorSupport::Skids => (MemberKind::Skid, "SK"),
    };
    let supports = if piers_used {
        pier_x.clone()
    } else {
        centers.clone()
    };
    let over = if piers_used { "pier" } else { "joist" };
    let (ply_joints, shared) = beam_joints(
        floor.beam,
        floor.beam_plies,
        width,
        &supports,
        over,
        stock,
        rules,
    )
    .map_err(|r| fail(r.replacen("the beam", &format!("the {}", kind.name()), 1)))?;
    if shared {
        warnings.push(format!(
            "the {} plies are spliced over the same {over}; more piers would let the joints stagger",
            kind.name(),
        ));
    }
    for (i, &y) in beam_y.iter().enumerate() {
        let (beam, beam_splices) = beam_line(
            &format!("floor-{prefix}{}", i + 1),
            kind,
            floor.beam,
            width,
            y,
            &ply_joints,
        );
        members.extend(beam);
        splices.extend(beam_splices);
    }
    splices.sort_by_key(|s| s.at);
    splices.dedup();

    // Posts: one on each pier, centered under its beam.
    if floor.support == FloorSupport::Posts {
        let post = floor.post_size();
        let (pw, pd) = (post.actual_width(), post.actual_thickness());
        for (i, pier) in piers.iter().enumerate() {
            let mut p = placed(
                format!("floor-P{:02}", i + 1),
                MemberKind::Post,
                post,
                Rect {
                    x: pier.x - half(pw),
                    y: pier.y - half(pd),
                    w: pw,
                    h: pd,
                },
                false,
            );
            p.member.length = floor.post_length();
            members.push(p);
        }
    }

    // Subfloor: sheets run across the joists in rows, every other row
    // starting with a half sheet so end joints stagger.
    let sheet = sheets
        .iter()
        .find(|s| s.thickness == floor.subfloor)
        .cloned()
        .unwrap_or_else(|| SheetStock::standard(floor.subfloor));
    let subfloor: Vec<PlacedSheet> =
        sheet_rows(width, depth, sheet.length, sheet.width, rules.layout_from)
            .into_iter()
            .enumerate()
            .map(|(i, rect)| PlacedSheet {
                piece: SheetPiece {
                    label: format!("floor-SF{:02}", i + 1),
                    used_for: SheetUse::Subfloor,
                    thickness: sheet.thickness,
                    length: rect.w,
                    width: rect.h,
                },
                rect,
            })
            .collect();
    let mut missed: Vec<Length> = subfloor
        .iter()
        .flat_map(|s| [s.rect.x, s.rect.right()])
        .filter(|&x| x > Length::ZERO && x < width)
        .filter(|&x| !joist_x.iter().any(|&j| x >= j && x <= j + t))
        .collect();
    missed.sort();
    missed.dedup();
    if !missed.is_empty() {
        let list: Vec<String> = missed.iter().map(|x| format!("{x:#}")).collect();
        warnings.push(format!(
            "subfloor joints at {} miss the joists; {oc:#} spacing from the layout end lines them up",
            list.join(", ")
        ));
    }

    let marks = joist_marks("floor-J", &joist_x, t, width, rules.layout_from);

    Ok(FramedFloor {
        width,
        depth,
        members,
        piers,
        bays,
        marks,
        subfloor,
        splices,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sticks_core::{Nominal, Project};

    fn l(s: &str) -> Length {
        s.parse().unwrap()
    }

    fn shed_floor(extra: &str) -> FramedFloor {
        let p = Project::from_toml(&format!(
            r#"
name = "Shed"
[building]
width = "18'"
depth = "14'"
height = "10'"
[floor]
beams = 3
{extra}
[[stock]]
size = "2x6"
lengths = ["8'", "10'", "12'", "14'", "16'"]
"#
        ))
        .unwrap();
        frame_floor(
            p.floor.as_ref().unwrap(),
            p.building.as_ref().unwrap(),
            &p.rules,
            &p.stock,
            &p.sheets,
        )
        .unwrap()
    }

    fn of_kind(f: &FramedFloor, kind: MemberKind) -> Vec<&Placed> {
        f.members.iter().filter(|p| p.member.kind == kind).collect()
    }

    #[test]
    fn frames_the_reference_shed_floor() {
        let f = shed_floor("");
        // 18' at 16" on center: two end joists and 13 on layout.
        let joists = of_kind(&f, MemberKind::Joist);
        assert_eq!(joists.len(), 15);
        assert!(joists.iter().all(|j| j.member.length == l("165")));
        assert_eq!(joists[1].rect.x, l("15-1/4"));
        // Bays are 14-1/2" clear between layout joists, narrower at the end.
        assert_eq!(f.bays[1].width, l("14-1/2"));
        assert_eq!(f.bays[0].width, l("13-3/4"));
        assert_eq!(f.bays.last().unwrap().width, l("5-3/4"));

        // Rims split over the joist centered at 16'.
        let rims = of_kind(&f, MemberKind::RimJoist);
        let lengths: Vec<_> = rims.iter().map(|r| r.member.length).collect();
        assert_eq!(lengths, [l("16'"), l("24"), l("16'"), l("24")]);

        // Three beams of 4x6, the middle one centered, with a row of blocks.
        let beams = of_kind(&f, MemberKind::Beam);
        let ys: Vec<_> = beams.iter().map(|b| b.rect.y).collect();
        assert!(ys.contains(&Length::ZERO));
        assert!(ys.contains(&(l("14'") - l("3-1/2"))));
        assert!(ys.contains(&(l("7'") - l("1-3/4"))));
        assert_eq!(of_kind(&f, MemberKind::Blocking).len(), f.bays.len());

        // Piers every 6' or less from 12" in: 12, 76, 140, 204 on each beam.
        assert_eq!(f.piers.len(), 12);
        assert_eq!(f.piers[1].x, l("76"));
        assert!(f
            .splices
            .iter()
            .any(|s| s.kind == MemberKind::Beam && s.at == l("140")));

        // 252 sq ft of subfloor in 4x8 rows, staggered by half a sheet.
        let area: i64 = f
            .subfloor
            .iter()
            .map(|s| s.rect.w.sixteenths() * s.rect.h.sixteenths())
            .sum();
        assert_eq!(area, l("18'").sixteenths() * l("14'").sixteenths());
        assert_eq!(f.subfloor[3].rect.w, l("4'"));
        assert!(f.warnings.is_empty(), "{:?}", f.warnings);
    }

    #[test]
    fn doubled_beams_stagger_their_joints() {
        let f = shed_floor("beam = \"2x8\"\nbeam_plies = 2");
        let beams = of_kind(&f, MemberKind::Beam);
        assert!(beams.iter().all(|b| b.member.size == Nominal::new(2, 8)));
        let first: Vec<_> = beams
            .iter()
            .filter(|b| b.member.label.starts_with("floor-BM1"))
            .map(|b| (b.member.label.as_str(), b.member.length))
            .collect();
        assert_eq!(
            first,
            [
                ("floor-BM1a-1", l("140")),
                ("floor-BM1a-2", l("76")),
                ("floor-BM1b-1", l("76")),
                ("floor-BM1b-2", l("140")),
            ]
        );
    }

    #[test]
    fn skids_have_no_piers() {
        let f = shed_floor("support = \"skids\"\nbeam = \"4x4\"");
        assert!(f.piers.is_empty());
        assert_eq!(of_kind(&f, MemberKind::Skid).len(), 6);
    }
}
