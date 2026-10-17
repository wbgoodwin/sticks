//! Lean-to roofs: rafters on center from the front wall to the back, each
//! with a birdsmouth over both walls and plumb-cut tails, and fascia across
//! the tails.
//!
//! Plan coordinates match the floor: `x` along the front from the left end,
//! `y` from the outside face of the front wall toward the back. The front
//! overhang sits at negative `y`.
//!
//! Section geometry, front to back: the rafter's bottom edge rests on the
//! front wall's cap plate at its outside face and on the back wall's cap
//! plate at its inside face. Each birdsmouth's seat is the full depth of the
//! wall, and the side walls are rake walls whose tops meet the rafters.

use serde::Serialize;
use sticks_core::{
    longest_board, tenths_of_degree, Building, Cut, LayoutStart, Length, MemberKind, Nominal,
    Pitch, Roof, Rules, Stock,
};

use crate::layout::{half, on_center, pieces, placed, split_run, LayoutMark, Placed, Rect, Splice};
use crate::FramingError;

/// Heights the walls under a lean-to need, subfloor to top of cap plate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct LeanToHeights {
    pub front: Length,
    pub back: Length,
    /// Where the side walls meet the inside face of the front wall.
    pub side_front: Length,
    /// Where the side walls meet the inside face of the back wall.
    pub side_back: Length,
}

/// How to cut every rafter: they're all alike.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RafterDetail {
    pub size: Nominal,
    /// Plumb cut to plumb cut, along either edge.
    pub length: Length,
    pub pitch: Pitch,
    /// Plumb cuts and the heel cuts are this far from square.
    pub plumb_tenths_of_degree: i32,
    /// Level run from tail to tail.
    pub run: Length,
    /// The rafter's depth measured plumb.
    pub plumb_depth: Length,
    /// Level seat cut, the full depth of the wall.
    pub seat: Length,
    /// Plumb heel cut.
    pub heel: Length,
    /// Along the bottom edge from the upper (front) end to each heel.
    pub front_heel_at: Length,
    pub back_heel_at: Length,
    pub front_overhang: Length,
    pub back_overhang: Length,
}

/// Everything generated for the roof.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FramedRoof {
    pub width: Length,
    pub depth: Length,
    pub heights: LeanToHeights,
    /// Rafters, then front and back fascia, in plan.
    pub members: Vec<Placed>,
    pub rafter: RafterDetail,
    /// Rafters, ordered from the layout end, as marked on the cap plates.
    pub marks: Vec<LayoutMark>,
    pub splices: Vec<Splice>,
    pub warnings: Vec<String>,
}

/// The wall heights that put every rafter's bottom edge on both walls' cap
/// plates at `pitch`. `wall_depth` is the actual width of the wall studs.
pub fn lean_to_heights(building: &Building, pitch: Pitch, wall_depth: Length) -> LeanToHeights {
    let front = building.height;
    LeanToHeights {
        front,
        back: front - pitch.rise_over(building.depth - wall_depth),
        side_front: front - pitch.rise_over(wall_depth),
        side_back: front - pitch.rise_over(building.depth - wall_depth),
    }
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
    let d = size.actual_width();
    let wall_depth = rules.wall_lumber.actual_width();
    let (width, depth) = (building.width, building.depth);
    let pitch = roof.pitch;
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
    let heights = lean_to_heights(building, pitch, wall_depth);

    let run = roof.front_overhang + depth + roof.back_overhang;
    let length = pitch.slope_length(run);
    let angle = tenths_of_degree(pitch.rise, Pitch::RUN);
    let heel = pitch.rise_over(wall_depth);
    let birdsmouth = |heel_at: Length| Cut::Birdsmouth {
        seat: wall_depth,
        heel,
        heel_at,
    };
    let rafter = RafterDetail {
        size,
        length,
        pitch,
        plumb_tenths_of_degree: angle,
        run,
        plumb_depth: pitch.slope_length(d),
        seat: wall_depth,
        heel,
        front_heel_at: pitch.slope_length(roof.front_overhang + wall_depth),
        back_heel_at: pitch.slope_length(roof.front_overhang + depth),
        front_overhang: roof.front_overhang,
        back_overhang: roof.back_overhang,
    };
    let mut warnings = Vec::new();
    if heel * 3 > d {
        warnings.push(format!(
            "the birdsmouth heel is {heel:#}, more than a third of the {size} rafter's depth"
        ));
    }

    let mut members = Vec::new();
    let mut rafter_x = vec![Length::ZERO];
    rafter_x.extend(on_center(width, oc, t, rules.layout_from));
    rafter_x.push(width - t);
    let plumb = Cut::Plumb {
        tenths_of_degree: angle,
    };
    for (i, &x) in rafter_x.iter().enumerate() {
        let mut p = placed(
            format!("roof-R{:02}", i + 1),
            MemberKind::Rafter,
            size,
            Rect {
                x,
                y: -roof.front_overhang,
                w: t,
                h: run,
            },
            true,
        );
        p.member.length = length;
        p.member.cuts = vec![
            plumb,
            birdsmouth(rafter.front_heel_at),
            birdsmouth(rafter.back_heel_at),
            plumb,
        ];
        members.push(p);
    }

    // Fascia across the tails, spliced over a rafter when too long.
    let fascia = roof
        .fascia
        .unwrap_or(Nominal::new(size.thickness, size.width.saturating_add(2)));
    let ft = fascia.actual_thickness();
    if fascia.actual_width() < rafter.plumb_depth {
        warnings.push(format!(
            "{fascia} fascia ({:#}) won't cover the rafter tails' {:#} plumb cut",
            fascia.actual_width(),
            rafter.plumb_depth
        ));
    }
    let centers: Vec<Length> = rafter_x.iter().map(|&x| x + half(t)).collect();
    let longest = longest_board(stock, fascia, rules.end_trim);
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
            "the fascia is longer than the longest {fascia} stock ({longest}) and no rafter lands where it could be spliced; add longer stock"
        ))
    })?;
    let mut splices = Vec::new();
    for (prefix, y) in [
        ("FF", -roof.front_overhang - ft),
        ("BF", depth + roof.back_overhang),
    ] {
        for (n, (a, b)) in pieces(Length::ZERO, width, &joints).into_iter().enumerate() {
            members.push(placed(
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
        splices.extend(joints.iter().map(|&at| Splice {
            kind: MemberKind::Fascia,
            at,
        }));
    }

    let mut marks: Vec<LayoutMark> = members
        .iter()
        .filter(|p| p.member.kind == MemberKind::Rafter)
        .map(|p| LayoutMark {
            label: p.member.label.clone(),
            kind: MemberKind::Rafter,
            at: match rules.layout_from {
                LayoutStart::Left => p.rect.x,
                LayoutStart::Right => width - p.rect.right(),
            },
        })
        .collect();
    marks.sort_by_key(|m| m.at);

    Ok(FramedRoof {
        width,
        depth,
        heights,
        members,
        rafter,
        marks,
        splices,
        warnings,
    })
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

        let d = &r.rafter;
        assert_eq!(d.run, l("192"));
        // 192 * 1.030776 = 197.909, nearest 197-15/16.
        assert_eq!(d.length, l("197-15/16"));
        assert_eq!(d.plumb_tenths_of_degree, 140);
        assert_eq!(d.heel, l("7/8"));
        assert_eq!(d.seat, l("3-1/2"));
        // 15-1/2" and 180" of run to the heels.
        assert_eq!(d.front_heel_at, l("16"));
        assert_eq!(d.back_heel_at, l("185-9/16"));
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
        assert_eq!(r.rafter.heel, l("2-5/16"));
        assert!(
            r.warnings[0].contains("more than a third"),
            "{:?}",
            r.warnings
        );
    }
}
