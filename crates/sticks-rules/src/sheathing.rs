//! Sheathing: sheets nailed over the outside of the walls and the top of the
//! roof, with door and window openings cut out.
//!
//! Each face is laid out flat in its own coordinates. A wall face is its
//! elevation seen from outside: `x` from the wall's left end, `y` up from the
//! top of the subfloor. Wall sheets stand on end in columns from the layout
//! end, each column's edges on stud centers, with a cut piece above the full
//! sheet where the wall is taller than one. A roof face runs `x` along the
//! roof from its left end and `y` up the slope from the tails; sheets run
//! across the rafters in rows from the eave, staggered like the subfloor.
//!
//! The side walls' sheathing runs past both ends to cover the ends of the
//! front and back walls, and up to the top of the end rafters.

use serde::Serialize;
use sticks_core::{
    LayoutStart, Length, MemberKind, Rules, Sheathing, SheetPiece, SheetStock, SheetUse,
};

use crate::layout::{half, sheet_rows, split_run, Rect};
use crate::porch::FramedPorch;
use crate::roof::{FramedRoof, RoofPlane};
use crate::wall::FramedWall;
use crate::FramingError;

/// One piece of sheathing and where it goes on its face.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SheathingPanel {
    pub piece: SheetPiece,
    /// Bounding box in the face's coordinates.
    pub rect: Rect,
    /// Corners in order when the piece isn't a plain rectangle, such as one
    /// cut to a raked or gable top. Empty when `rect` is the shape.
    pub outline: Vec<(Length, Length)>,
    /// Openings cut out of it, in the face's coordinates.
    pub holes: Vec<Rect>,
}

/// One sheathed face: a wall or a roof plane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SheathedFace {
    /// The wall's id, or the roof plane's name.
    pub name: String,
    pub thickness: Length,
    /// The roof plane, for roof faces.
    pub plane: Option<RoofPlane>,
    /// The whole face's outline, in its coordinates.
    pub outline: Vec<(Length, Length)>,
    pub panels: Vec<SheathingPanel>,
}

/// Everything sheathed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct FramedSheathing {
    pub walls: Vec<SheathedFace>,
    pub roofs: Vec<SheathedFace>,
    pub warnings: Vec<String>,
}

impl FramedSheathing {
    pub fn panels(&self) -> impl Iterator<Item = &SheathingPanel> {
        self.walls.iter().chain(&self.roofs).flat_map(|f| &f.panels)
    }
}

/// What a building wall's sheathing needs to know about the wall.
pub struct WallFace<'a> {
    pub wall: &'a FramedWall,
    /// The face's left and right ends, in the wall's `x`.
    pub from: Length,
    pub to: Length,
    /// The top of the face as `(x, y)` points, left to right.
    pub top: Vec<(Length, Length)>,
}

fn stock_for(sheets: &[SheetStock], thickness: Length) -> SheetStock {
    sheets
        .iter()
        .find(|s| s.thickness == thickness)
        .cloned()
        .unwrap_or_else(|| SheetStock::standard(thickness))
}

/// Sheathes `walls` (from `bottom`, the bottom of the floor's rim, up) and
/// the roof and porch roof planes, at the thicknesses `sheathing` gives.
pub fn frame_sheathing(
    sheathing: &Sheathing,
    walls: &[WallFace],
    roof: Option<&FramedRoof>,
    porch: Option<&FramedPorch>,
    bottom: Length,
    rules: &Rules,
    sheets: &[SheetStock],
) -> Result<FramedSheathing, FramingError> {
    let fail = |reason: String| FramingError {
        part: "sheathing".to_string(),
        reason,
    };
    for t in [sheathing.wall, sheathing.roof].into_iter().flatten() {
        if t <= Length::ZERO {
            return Err(fail(
                "a sheathing thickness must be more than zero".to_string(),
            ));
        }
    }
    let mut out = FramedSheathing::default();
    if let Some(thickness) = sheathing.wall {
        let stock = stock_for(sheets, thickness);
        for face in walls {
            out.walls.push(wall_face(
                face,
                thickness,
                &stock,
                bottom,
                rules,
                &mut out.warnings,
            ));
        }
    }
    if let Some(thickness) = sheathing.roof {
        let stock = stock_for(sheets, thickness);
        let mut n = 0;
        let planes = roof
            .into_iter()
            .flat_map(|r| r.planes.iter().map(|p| ("roof", p)))
            .chain(porch.map(|p| ("porch", &p.plane)));
        for (part, plane) in planes {
            if part == "porch" {
                n = 0;
            }
            let face = roof_face(plane, part, &mut n, thickness, &stock, rules);
            out.roofs.push(face);
        }
    }
    Ok(out)
}

fn roof_face(
    plane: &RoofPlane,
    part: &str,
    n: &mut usize,
    thickness: Length,
    stock: &SheetStock,
    rules: &Rules,
) -> SheathedFace {
    let (w, s) = (plane.width, plane.slope_length());
    let panels = sheet_rows(w, s, stock.length, stock.width, rules.layout_from)
        .into_iter()
        .map(|rect| {
            *n += 1;
            SheathingPanel {
                piece: SheetPiece {
                    label: format!("{part}-SH{n:02}"),
                    used_for: SheetUse::RoofSheathing,
                    thickness,
                    length: rect.w,
                    width: rect.h,
                },
                rect,
                outline: Vec::new(),
                holes: Vec::new(),
            }
        })
        .collect();
    SheathedFace {
        name: plane.name.clone(),
        thickness,
        plane: Some(plane.clone()),
        outline: vec![
            (Length::ZERO, Length::ZERO),
            (w, Length::ZERO),
            (w, s),
            (Length::ZERO, s),
        ],
        panels,
    }
}

/// The top line's height at `x`, interpolated between its points.
fn top_at(top: &[(Length, Length)], x: Length) -> Length {
    let Some(&(x0, y0)) = top.first() else {
        return Length::ZERO;
    };
    if x <= x0 {
        return y0;
    }
    for w in top.windows(2) {
        let ((ax, ay), (bx, by)) = (w[0], w[1]);
        if x <= bx {
            if bx == ax {
                return by;
            }
            return ay + (by - ay).mul_div((x - ax).sixteenths(), (bx - ax).sixteenths());
        }
    }
    top.last().map_or(Length::ZERO, |&(_, y)| y)
}

/// Where the top line crosses height `level`, strictly between `a` and `b`.
fn crossings(top: &[(Length, Length)], level: Length, a: Length, b: Length) -> Vec<Length> {
    let mut out = Vec::new();
    for w in top.windows(2) {
        let ((ax, ay), (bx, by)) = (w[0], w[1]);
        if (ay < level) != (by < level) && ay != by {
            let x = ax + (bx - ax).mul_div((level - ay).sixteenths(), (by - ay).sixteenths());
            if x > a && x < b {
                out.push(x);
            }
        }
    }
    out
}

/// The part of the face under `top`, between `a` and `b` and from `lo` to
/// `hi`, as a polygon; `None` when nothing of the face is there.
fn region(
    top: &[(Length, Length)],
    a: Length,
    b: Length,
    lo: Length,
    hi: Length,
) -> Option<Vec<(Length, Length)>> {
    let mut xs = vec![a, b];
    xs.extend(top.iter().map(|&(x, _)| x).filter(|&x| x > a && x < b));
    xs.extend(crossings(top, lo, a, b));
    xs.extend(crossings(top, hi, a, b));
    xs.sort();
    xs.dedup();
    if xs.iter().all(|&x| top_at(top, x) <= lo) {
        return None;
    }
    let mut poly = vec![(a, lo), (b, lo)];
    for &x in xs.iter().rev() {
        let y = top_at(top, x).clamp(lo, hi);
        if poly.last() != Some(&(x, y)) {
            poly.push((x, y));
        }
    }
    if poly.first() == poly.last() {
        poly.pop();
    }
    Some(poly)
}

fn bounds(poly: &[(Length, Length)]) -> Rect {
    let x0 = poly.iter().map(|p| p.0).min().unwrap_or_default();
    let x1 = poly.iter().map(|p| p.0).max().unwrap_or_default();
    let y0 = poly.iter().map(|p| p.1).min().unwrap_or_default();
    let y1 = poly.iter().map(|p| p.1).max().unwrap_or_default();
    Rect {
        x: x0,
        y: y0,
        w: x1 - x0,
        h: y1 - y0,
    }
}

fn intersect(a: Rect, b: Rect) -> Option<Rect> {
    let x0 = a.x.max(b.x);
    let x1 = a.right().min(b.right());
    let y0 = a.y.max(b.y);
    let y1 = a.top().min(b.top());
    (x1 > x0 && y1 > y0).then_some(Rect {
        x: x0,
        y: y0,
        w: x1 - x0,
        h: y1 - y0,
    })
}

fn wall_face(
    face: &WallFace,
    thickness: Length,
    stock: &SheetStock,
    bottom: Length,
    rules: &Rules,
    warnings: &mut Vec<String>,
) -> SheathedFace {
    let wall = face.wall;
    let (a, b) = (face.from, face.to);
    let column = stock.width;
    // Column joints on stud centers, from the layout end.
    let t = rules.wall_lumber.actual_thickness();
    let mut centers: Vec<Length> = wall
        .members
        .iter()
        .filter(|p| {
            matches!(
                p.member.kind,
                MemberKind::Stud | MemberKind::KingStud | MemberKind::GableStud
            )
        })
        .map(|p| p.rect.x + half(t))
        .collect();
    centers.sort();
    centers.dedup();
    let flip = |x: Length| match rules.layout_from {
        LayoutStart::Left => x,
        LayoutStart::Right => a + b - x,
    };
    let candidates: Vec<Length> = centers.iter().map(|&c| flip(c)).collect();
    let fits = |s: Length| s <= column;
    let joints = split_run(a, b, fits, &candidates, &[], Length::ZERO).unwrap_or_else(|| {
        warnings.push(format!(
            "wall {}: no stud lands where some sheathing joints fall; add blocking behind them",
            wall.id
        ));
        let mut j = Vec::new();
        let mut x = a + column;
        while x < b {
            j.push(x);
            x += column;
        }
        j
    });
    let mut joints: Vec<Length> = joints.into_iter().map(flip).collect();
    joints.sort();
    let mut edges = vec![a];
    edges.extend(joints);
    edges.push(b);

    let top = &face.top;
    let highest = top.iter().map(|p| p.1).max().unwrap_or(bottom);
    let holes: Vec<Rect> = wall
        .openings
        .iter()
        .filter(|o| o.kind != sticks_core::OpeningKind::Penetration)
        .map(|o| o.rect)
        .collect();
    let mut panels = Vec::new();
    for col in edges.windows(2) {
        let (ca, cb) = (col[0], col[1]);
        let mut lo = bottom;
        while lo < highest {
            let hi = lo + stock.length;
            if let Some(poly) = region(top, ca, cb, lo, hi) {
                let rect = bounds(&poly);
                let plain = poly.len() == 4
                    && poly.iter().all(|&(x, y)| {
                        (x == rect.x || x == rect.right()) && (y == rect.y || y == rect.top())
                    });
                panels.push(SheathingPanel {
                    piece: SheetPiece {
                        label: format!("{}-SH{:02}", wall.id, panels.len() + 1),
                        used_for: SheetUse::WallSheathing,
                        thickness,
                        length: rect.w.max(rect.h),
                        width: rect.w.min(rect.h),
                    },
                    rect,
                    outline: if plain { Vec::new() } else { poly },
                    holes: holes.iter().filter_map(|&h| intersect(h, rect)).collect(),
                });
            }
            lo = hi;
        }
    }
    let mut outline = vec![(a, bottom), (b, bottom)];
    outline.extend(top.iter().rev().copied());
    SheathedFace {
        name: wall.id.clone(),
        thickness,
        plane: None,
        outline,
        panels,
    }
}

/// A panel as hole-free pieces, for drawing in 3D: the strips beside each
/// hole run full height, and the strips across a hole stop at it.
pub fn solid_pieces(panel: &SheathingPanel) -> Vec<Vec<(Length, Length)>> {
    let outline = if panel.outline.is_empty() {
        let r = panel.rect;
        vec![
            (r.x, r.y),
            (r.right(), r.y),
            (r.right(), r.top()),
            (r.x, r.top()),
        ]
    } else {
        panel.outline.clone()
    };
    if panel.holes.is_empty() {
        return vec![outline];
    }
    // The outline's top, left to right, from its points above the bottom.
    let r = panel.rect;
    let mut top: Vec<(Length, Length)> =
        outline.iter().copied().filter(|&(_, y)| y > r.y).collect();
    top.sort();
    if top.is_empty() {
        return Vec::new();
    }
    let mut xs = vec![r.x, r.right()];
    for h in &panel.holes {
        xs.extend([h.x, h.right()]);
    }
    xs.sort();
    xs.dedup();
    let mut out = Vec::new();
    for s in xs.windows(2) {
        let (a, b) = (s[0], s[1]);
        let mut cuts: Vec<(Length, Length)> = panel
            .holes
            .iter()
            .filter(|h| h.x <= a && h.right() >= b)
            .map(|h| (h.y, h.top()))
            .collect();
        cuts.sort();
        let mut lo = r.y;
        for (h0, h1) in cuts.into_iter().chain([(r.top(), r.top())]) {
            if h0 > lo {
                if let Some(p) = region(&top, a, b, lo, h0) {
                    out.push(p);
                }
            }
            lo = lo.max(h1);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(s: &str) -> Length {
        s.parse().unwrap()
    }

    #[test]
    fn regions_follow_a_gable_top() {
        let top = vec![(l("0"), l("100")), (l("48"), l("124")), (l("96"), l("100"))];
        let p = region(&top, l("24"), l("72"), l("96"), l("192")).unwrap();
        assert_eq!(
            p,
            [
                (l("24"), l("96")),
                (l("72"), l("96")),
                (l("72"), l("112")),
                (l("48"), l("124")),
                (l("24"), l("112")),
            ]
        );
        assert!(region(&top, l("0"), l("48"), l("124"), l("200")).is_none());
    }

    #[test]
    fn holes_split_a_panel_into_solid_strips() {
        let panel = SheathingPanel {
            piece: SheetPiece {
                label: "x".to_string(),
                used_for: SheetUse::WallSheathing,
                thickness: l("7/16"),
                length: l("96"),
                width: l("48"),
            },
            rect: Rect {
                x: l("0"),
                y: l("0"),
                w: l("48"),
                h: l("96"),
            },
            outline: Vec::new(),
            holes: vec![Rect {
                x: l("10"),
                y: l("0"),
                w: l("38"),
                h: l("80"),
            }],
        };
        let strips = solid_pieces(&panel);
        // Left of the door, full height; over the door, above its head.
        assert_eq!(strips.len(), 2);
        assert_eq!(bounds(&strips[0]).w, l("10"));
        assert_eq!(bounds(&strips[1]).y, l("80"));
    }
}
