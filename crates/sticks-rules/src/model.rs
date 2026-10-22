//! The framed project put together in three dimensions, for the 3D view.
//!
//! World coordinates are inches: `x` along the front of the building from its
//! left end, `y` from the outside face of the front wall toward the back, and
//! `z` up from the top of the subfloor, where the walls stand. The floor
//! framing hangs below `z = 0` and the roof sits on the walls' cap plates.
//!
//! Every piece is a prism: a flat profile drawn in the plane of two axes `u`
//! and `v` from `origin`, pushed `depth` inches along `u × v`. A stud is its
//! elevation rectangle pushed through the wall; a rafter is its side profile,
//! birdsmouths included, pushed across its thickness; a sheet of sheathing
//! is its outline on the face it covers, pushed out by its thickness.

use serde::Serialize;
use sticks_core::{Length, Member, MemberKind, Project};

use crate::building::{FramedProject, SIDES};
use crate::deck::FramedDeck;
use crate::floor::{FramedFloor, Pier};
use crate::layout::{Placed, Rect};
use crate::porch::FramedPorch;
use crate::roof::FramedRoof;
use crate::sheathing::{solid_pieces, SheathedFace};
use crate::wall::FramedWall;

/// What a solid is: a framing member, or something drawn for context.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum SolidKind {
    Member(MemberKind),
    Other(OtherKind),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OtherKind {
    Pier,
    Subfloor,
    Sheathing,
}

/// One piece of the model.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Solid {
    pub label: String,
    /// The part it belongs to: a wall id, `floor`, `roof` or `porch`.
    pub part: String,
    pub kind: SolidKind,
    /// The member it shows, for framing members.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub member: Option<Member>,
    /// Corners in the `u`, `v` plane, in inches.
    pub profile: Vec<[f64; 2]>,
    pub origin: [f64; 3],
    pub u: [f64; 3],
    pub v: [f64; 3],
    /// How far the profile is pushed along `u × v`.
    pub depth: f64,
}

/// Every member of `framed` placed in the building, plus the piers,
/// subfloor and sheathing. Walls that aren't part of a building are stood in a row along
/// the front line when the project has no building, and left out otherwise,
/// since nothing says where they go.
pub fn model(project: &Project, framed: &FramedProject) -> Vec<Solid> {
    let wall_depth = inches(project.rules.wall_lumber.actual_width());
    let mut out = Vec::new();
    match &project.building {
        Some(b) => {
            let (w, d) = (inches(b.width), inches(b.depth));
            for wall in framed.walls.iter().flatten() {
                let Some((origin, u)) = wall_place(&wall.id, w, d, wall_depth) else {
                    continue;
                };
                wall_solids(&mut out, wall, origin, u, wall_depth);
            }
            if let (Some(Ok(floor)), Some(f)) = (&framed.floor, &project.floor) {
                floor_solids(
                    &mut out,
                    floor,
                    inches(f.subfloor),
                    inches(f.joist.actual_width()),
                );
            }
            if let Some(Ok(roof)) = &framed.roof {
                roof_solids(&mut out, roof, wall_depth);
            }
            if let Some(Ok(porch)) = &framed.porch {
                porch_solids(&mut out, porch);
            }
            if let Some(Ok(sheathing)) = &framed.sheathing {
                for face in &sheathing.walls {
                    if let Some((origin, u)) = wall_place(&face.name, w, d, wall_depth) {
                        let out_face = along(origin, cross(u, UP), wall_depth);
                        sheathing_solids(&mut out, face, &face.name, out_face, u, UP, false);
                    }
                }
                for face in &sheathing.roofs {
                    roof_sheathing(&mut out, face);
                }
            }
        }
        None => {
            let mut x = 0.0;
            for wall in framed.walls.iter().flatten() {
                wall_solids(
                    &mut out,
                    wall,
                    [x, wall_depth, 0.0],
                    [1.0, 0.0, 0.0],
                    wall_depth,
                );
                x += inches(wall.length) + 48.0;
            }
        }
    }
    if let Some(Ok(deck)) = &framed.deck {
        // Beside a building, the deck stands to its right with its top level
        // with the floor; on its own, the ground is at zero.
        let (dx, dz) = match &project.building {
            Some(b) => (inches(b.width) + 48.0, -inches(deck.heights.top)),
            None => (0.0, 0.0),
        };
        let start = out.len();
        deck_solids(&mut out, deck);
        for s in &mut out[start..] {
            s.origin[0] += dx;
            s.origin[2] += dz;
        }
    }
    out
}

/// A deck on its own footings: decking on joists and rims, the beams under
/// them or level with them, posts, and round concrete footings from below
/// the ground up to the pier tops.
fn deck_solids(out: &mut Vec<Solid>, deck: &FramedDeck) {
    let h = &deck.heights;
    let (top, jt, jb) = (inches(h.top), inches(h.joist_top), inches(h.joist_bottom));
    let (bt, bb, pt) = (
        inches(h.beam_top),
        inches(h.beam_bottom),
        inches(h.pier_top),
    );
    for p in &deck.members {
        let (lo, hi) = match p.member.kind {
            MemberKind::Decking => (jt, top),
            MemberKind::Beam => (bb, bt),
            MemberKind::Post => (pt, bb),
            _ => (jb, jt),
        };
        out.push(plan_solid(p, "deck", lo, hi));
    }
    for (i, (pier, footing)) in deck.piers.iter().zip(&deck.footings).enumerate() {
        let r = inches(footing.diameter) / 2.0;
        let (cx, cy) = (inches(pier.x), inches(pier.y));
        let profile = (0..16)
            .map(|k| {
                let a = k as f64 * std::f64::consts::PI / 8.0;
                [cx + r * a.cos(), cy + r * a.sin()]
            })
            .collect();
        let bottom = -inches(footing.depth);
        out.push(Solid {
            label: format!("deck footing {}", i + 1),
            part: "deck".to_string(),
            kind: SolidKind::Other(OtherKind::Pier),
            member: None,
            profile,
            origin: [0.0, 0.0, bottom],
            u: [1.0, 0.0, 0.0],
            v: [0.0, 1.0, 0.0],
            depth: pt - bottom,
        });
    }
}

/// Where a building wall stands: the origin on its inside face at its left
/// end, and `u` along the wall as seen from outside, so `u × up` points
/// outside.
fn wall_place(id: &str, w: f64, d: f64, wall_depth: f64) -> Option<([f64; 3], [f64; 3])> {
    Some(match id {
        "front" => ([0.0, wall_depth, 0.0], [1.0, 0.0, 0.0]),
        "back" => ([w, d - wall_depth, 0.0], [-1.0, 0.0, 0.0]),
        "left" => ([wall_depth, d - wall_depth, 0.0], [0.0, -1.0, 0.0]),
        "right" => ([w - wall_depth, wall_depth, 0.0], [0.0, 1.0, 0.0]),
        _ => return None,
    })
}

fn inches(l: Length) -> f64 {
    l.as_inches_f64()
}

fn corners(r: &Rect) -> Vec<[f64; 2]> {
    let (x, y, w, h) = (inches(r.x), inches(r.y), inches(r.w), inches(r.h));
    vec![[x, y], [x + w, y], [x + w, y + h], [x, y + h]]
}

fn member_solid(p: &Placed, part: &str) -> Solid {
    Solid {
        label: p.member.label.clone(),
        part: part.to_string(),
        kind: SolidKind::Member(p.member.kind),
        member: Some(p.member.clone()),
        profile: Vec::new(),
        origin: [0.0; 3],
        u: [1.0, 0.0, 0.0],
        v: [0.0, 1.0, 0.0],
        depth: 0.0,
    }
}

const UP: [f64; 3] = [0.0, 0.0, 1.0];

fn along(origin: [f64; 3], dir: [f64; 3], by: f64) -> [f64; 3] {
    [
        origin[0] + dir[0] * by,
        origin[1] + dir[1] * by,
        origin[2] + dir[2] * by,
    ]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// A wall's elevation pushed through the wall. Header plies that share a
/// rectangle in the elevation sit side by side, flush with the outside face.
fn wall_solids(out: &mut Vec<Solid>, wall: &FramedWall, origin: [f64; 3], u: [f64; 3], depth: f64) {
    let outward = cross(u, UP);
    let part = wall.id.clone();
    for p in &wall.members {
        let plies: Vec<&Placed> = wall.members.iter().filter(|o| o.rect == p.rect).collect();
        let (start, thick) = if plies.len() > 1 {
            let t = inches(p.member.size.actual_thickness());
            let i = plies.iter().position(|o| std::ptr::eq(*o, p)).unwrap_or(0);
            (depth - t * (plies.len() - i) as f64, t)
        } else {
            (0.0, depth)
        };
        let profile = if p.outline.is_empty() {
            corners(&p.rect)
        } else {
            p.outline
                .iter()
                .map(|&(x, y)| [inches(x), inches(y)])
                .collect()
        };
        out.push(Solid {
            profile,
            origin: along(origin, outward, start),
            u,
            v: UP,
            depth: thick,
            ..member_solid(p, &part)
        });
    }
}

/// Plan rectangles stood up: joists and rims under the subfloor, beams under
/// the joists, any posts under the beams, and piers under it all.
fn floor_solids(out: &mut Vec<Solid>, floor: &FramedFloor, subfloor: f64, joist: f64) {
    let joist_bottom = -subfloor - joist;
    let beam_bottom = floor
        .members
        .iter()
        .filter(|p| matches!(p.member.kind, MemberKind::Beam | MemberKind::Skid))
        .map(|p| joist_bottom - inches(p.member.size.actual_width()))
        .fold(joist_bottom, f64::min);
    let mut pier_top = beam_bottom;
    for p in &floor.members {
        let mut height = inches(p.member.size.actual_width());
        let bottom = match p.member.kind {
            MemberKind::Beam | MemberKind::Skid => joist_bottom - height,
            MemberKind::Post => {
                height = inches(p.member.length);
                pier_top = pier_top.min(beam_bottom - height);
                beam_bottom - height
            }
            _ => -subfloor - height,
        };
        out.push(Solid {
            profile: corners(&p.rect),
            origin: [0.0, 0.0, bottom],
            depth: height,
            ..member_solid(p, "floor")
        });
    }
    for s in &floor.subfloor {
        out.push(Solid {
            label: s.piece.label.clone(),
            part: "floor".to_string(),
            kind: SolidKind::Other(OtherKind::Subfloor),
            member: None,
            profile: corners(&s.rect),
            origin: [0.0, 0.0, -subfloor],
            u: [1.0, 0.0, 0.0],
            v: [0.0, 1.0, 0.0],
            depth: subfloor,
        });
    }
    pier_solids(out, &floor.piers, pier_top, "floor");
}

const PIER: f64 = 12.0;
const PIER_DROP: f64 = 16.0;

/// Concrete piers whose tops are at `top`.
fn pier_solids(out: &mut Vec<Solid>, piers: &[Pier], top: f64, part: &str) {
    for (i, pier) in piers.iter().enumerate() {
        let (x, y) = (inches(pier.x) - PIER / 2.0, inches(pier.y) - PIER / 2.0);
        out.push(Solid {
            label: format!("{part} pier {}", i + 1),
            part: part.to_string(),
            kind: SolidKind::Other(OtherKind::Pier),
            member: None,
            profile: vec![[x, y], [x + PIER, y], [x + PIER, y + PIER], [x, y + PIER]],
            origin: [0.0, 0.0, top - PIER_DROP],
            u: [1.0, 0.0, 0.0],
            v: [0.0, 1.0, 0.0],
            depth: PIER_DROP,
        });
    }
}

/// A plan rectangle stood up from `bottom` to `top`.
fn plan_solid(p: &Placed, part: &str, bottom: f64, top: f64) -> Solid {
    Solid {
        profile: corners(&p.rect),
        origin: [0.0, 0.0, bottom],
        depth: top - bottom,
        ..member_solid(p, part)
    }
}

/// A side profile in the `y`, `z` plane pushed across a member's thickness.
fn side_solid(p: &Placed, part: &str, profile: Vec<[f64; 2]>) -> Solid {
    Solid {
        profile,
        origin: [inches(p.rect.x), 0.0, 0.0],
        u: [0.0, 1.0, 0.0],
        v: UP,
        depth: inches(p.rect.w),
        ..member_solid(p, part)
    }
}

/// Rafters as side profiles with their birdsmouths, the ridge board and
/// collar ties for a gable, and fascia across the tails, flush with their
/// tops.
fn roof_solids(out: &mut Vec<Solid>, roof: &FramedRoof, wall_depth: f64) {
    let r = &roof.rafters[0];
    let slope = inches(r.pitch.rise) / 12.0;
    let depth = inches(roof.depth);
    let plumb = inches(r.plumb_depth);
    let front = inches(roof.heights.front);
    let back = inches(roof.heights.back);
    match roof.ridge {
        None => {
            // A lean-to: the bottom edge bears on the front wall's outside face.
            let fo = inches(r.upper_overhang);
            let bo = inches(r.overhang);
            let bottom = |y: f64| front - y * slope;
            let (tail, end) = (-fo, depth + bo);
            let profile = vec![
                [tail, bottom(tail)],
                [0.0, front],
                [wall_depth, front],
                [wall_depth, bottom(wall_depth)],
                [depth - wall_depth, back],
                [depth, back],
                [depth, bottom(depth)],
                [end, bottom(end)],
                [end, bottom(end) + plumb],
                [tail, bottom(tail) + plumb],
            ];
            for p in &roof.members {
                match p.member.kind {
                    MemberKind::Rafter => out.push(side_solid(p, "roof", profile.clone())),
                    _ => {
                        let top = if inches(p.rect.y) < 0.0 {
                            bottom(tail) + plumb
                        } else {
                            bottom(end) + plumb
                        };
                        let h = inches(p.member.size.actual_width());
                        out.push(plan_solid(p, "roof", top - h, top));
                    }
                }
            }
        }
        Some(ridge) => {
            let ry = inches(ridge.y);
            let details = &roof.rafters;
            let back_d = details.last().unwrap_or(r);
            let (fo, bo) = (inches(r.overhang), inches(back_d.overhang));
            // Front slope, bottom edge on the cap plate's inside corner;
            // the back slope is its mirror image.
            let bottom = |y: f64| front + (y - wall_depth) * slope;
            let profile = |overhang: f64| {
                let tail = -overhang;
                vec![
                    [tail, bottom(tail)],
                    [0.0, bottom(0.0)],
                    [0.0, front],
                    [wall_depth, front],
                    [ry, bottom(ry)],
                    [ry, bottom(ry) + plumb],
                    [tail, bottom(tail) + plumb],
                ]
            };
            let mirror = |pts: Vec<[f64; 2]>| -> Vec<[f64; 2]> {
                pts.into_iter().rev().map(|[y, z]| [depth - y, z]).collect()
            };
            let front_profile = profile(fo);
            let back_profile = mirror(profile(bo));
            for p in &roof.members {
                let y = inches(p.rect.y);
                match p.member.kind {
                    MemberKind::Rafter => {
                        let pr = if y < ry {
                            front_profile.clone()
                        } else {
                            back_profile.clone()
                        };
                        out.push(side_solid(p, "roof", pr));
                    }
                    MemberKind::Ridge => out.push(plan_solid(
                        p,
                        "roof",
                        inches(ridge.bottom),
                        inches(ridge.top),
                    )),
                    MemberKind::CollarTie => {
                        let (y1, y2) = (y, y + inches(p.rect.h));
                        let top = bottom(y1) + plumb;
                        let h = inches(p.member.size.actual_width());
                        out.push(side_solid(
                            p,
                            "roof",
                            vec![[y1, top - h], [y2, top - h], [y2, top], [y1, top]],
                        ));
                    }
                    _ => {
                        let tail = if y < 0.0 { -fo } else { -bo };
                        let top = bottom(tail) + plumb;
                        let h = inches(p.member.size.actual_width());
                        out.push(plan_solid(p, "roof", top - h, top));
                    }
                }
            }
        }
    }
}

/// Posts on piers, the beam on the posts, the ledger on the front wall, and
/// rafters from the ledger down over the beam to fascia at the tails.
fn porch_solids(out: &mut Vec<Solid>, porch: &FramedPorch) {
    let h = &porch.heights;
    let r = &porch.rafter;
    let slope = inches(r.pitch.rise) / 12.0;
    let plumb = inches(r.plumb_depth);
    let ledger_top = inches(h.ledger_top);
    let (beam_top, beam_bottom) = (inches(h.beam_top), inches(h.beam_bottom));
    let post_bottom = inches(h.post_bottom);
    let lt = inches(porch.plane.top.0).abs();
    let depth = inches(porch.depth);
    let inner = -depth + inches(r.seat);
    let tail = -depth - inches(r.overhang);
    // Bottom edge, dropping away from the ledger's face.
    let bottom = |y: f64| ledger_top - plumb - (-lt - y) * slope;
    let profile = vec![
        [tail, bottom(tail) + plumb],
        [tail, bottom(tail)],
        [-depth, bottom(-depth)],
        [-depth, beam_top],
        [inner, beam_top],
        [-lt, bottom(-lt)],
        [-lt, ledger_top],
    ];
    for p in &porch.members {
        let size = inches(p.member.size.actual_width());
        match p.member.kind {
            MemberKind::Post => out.push(plan_solid(p, "porch", post_bottom, beam_bottom)),
            MemberKind::Beam => out.push(plan_solid(p, "porch", beam_bottom, beam_top)),
            MemberKind::Ledger => out.push(plan_solid(p, "porch", ledger_top - size, ledger_top)),
            MemberKind::Rafter => out.push(side_solid(p, "porch", profile.clone())),
            _ => {
                let top = bottom(tail) + plumb;
                out.push(plan_solid(p, "porch", top - size, top));
            }
        }
    }
    pier_solids(out, &porch.piers, post_bottom, "porch");
    if let Some(deck) = &porch.deck {
        let h = &deck.heights;
        let (top, jt, jb, bb) = (
            inches(h.top),
            inches(h.joist_top),
            inches(h.joist_bottom),
            inches(h.beam_bottom),
        );
        for p in &deck.members {
            let (lo, hi) = match p.member.kind {
                MemberKind::Decking => (jt, top),
                MemberKind::Beam => (bb, jb),
                _ => (jb, jt),
            };
            out.push(plan_solid(p, "deck", lo, hi));
        }
        pier_solids(out, &deck.piers, bb, "deck");
    }
}

/// Sheathing panels on a face whose coordinates map to `origin + x u + y v`,
/// pushed out along `u × v` (or back toward it, when `inward`).
fn sheathing_solids(
    out: &mut Vec<Solid>,
    face: &SheathedFace,
    part: &str,
    origin: [f64; 3],
    u: [f64; 3],
    v: [f64; 3],
    inward: bool,
) {
    let t = inches(face.thickness);
    let n = cross(u, v);
    let origin = if inward { along(origin, n, -t) } else { origin };
    for panel in &face.panels {
        for piece in solid_pieces(panel) {
            out.push(Solid {
                label: panel.piece.label.clone(),
                part: part.to_string(),
                kind: SolidKind::Other(OtherKind::Sheathing),
                member: None,
                profile: piece.iter().map(|&(x, y)| [inches(x), inches(y)]).collect(),
                origin,
                u,
                v,
                depth: t,
            });
        }
    }
}

/// Roof sheathing on its plane, from the tails' top edge up the slope.
fn roof_sheathing(out: &mut Vec<Solid>, face: &SheathedFace) {
    let Some(plane) = &face.plane else {
        return;
    };
    let (ey, ez) = (inches(plane.eave.0), inches(plane.eave.1));
    let up = if plane.top.0 > plane.eave.0 {
        1.0
    } else {
        -1.0
    };
    let slope = inches(plane.pitch.rise) / 12.0;
    let k = 1.0 / (1.0 + slope * slope).sqrt();
    let v = [0.0, up * k, slope * k];
    let u = [1.0, 0.0, 0.0];
    // u × v points down on a slope that rises toward the front; push the
    // sheets back up onto the rafters there.
    let inward = cross(u, v)[2] < 0.0;
    let part = if plane.name == "porch roof" {
        "porch"
    } else {
        "roof"
    };
    sheathing_solids(out, face, part, [0.0, ey, ez], u, v, inward);
}

/// The smallest box around every solid, as `[min, max]` corners.
pub fn bounds(solids: &[Solid]) -> Option<[[f64; 3]; 2]> {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for s in solids {
        let n = cross(s.u, s.v);
        for &[a, b] in &s.profile {
            for k in [0.0, s.depth] {
                for i in 0..3 {
                    let c = s.origin[i] + s.u[i] * a + s.v[i] * b + n[i] * k;
                    lo[i] = lo[i].min(c);
                    hi[i] = hi[i].max(c);
                }
            }
        }
    }
    (lo[0] <= hi[0]).then_some([lo, hi])
}

/// Labels of the parts a project shows, in drawing order.
pub fn wall_ids(project: &Project) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    if project.building.is_some() {
        ids.extend(SIDES.iter().map(|s| s.to_string()));
    }
    ids.extend(
        project
            .walls
            .iter()
            .filter(|w| project.building.is_none() || !SIDES.contains(&w.id.as_str()))
            .map(|w| w.id.clone()),
    );
    ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::building::frame_project;

    fn shed() -> (Project, Vec<Solid>) {
        let p = Project::from_toml(include_str!("../../../examples/shed.toml")).unwrap();
        let f = frame_project(&p).unwrap();
        let m = model(&p, &f);
        (p, m)
    }

    fn corners_of(s: &Solid) -> Vec<[f64; 3]> {
        let n = cross(s.u, s.v);
        let mut out = Vec::new();
        for &[a, b] in &s.profile {
            for k in [0.0, s.depth] {
                out.push(std::array::from_fn(|i| {
                    s.origin[i] + s.u[i] * a + s.v[i] * b + n[i] * k
                }));
            }
        }
        out
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn every_member_is_in_the_model() {
        let p = Project::from_toml(include_str!("../../../examples/shed.toml")).unwrap();
        let f = frame_project(&p).unwrap();
        let m = model(&p, &f);
        let members = m.iter().filter(|s| s.member.is_some()).count();
        assert_eq!(members, f.members().len());
        assert!(m
            .iter()
            .any(|s| s.kind == SolidKind::Other(OtherKind::Pier)));
    }

    #[test]
    fn walls_sit_on_the_footprint() {
        let (_, m) = shed();
        let [lo, hi] = bounds(
            &m.iter()
                .filter(|s| SIDES.contains(&s.part.as_str()))
                .filter(|s| s.kind != SolidKind::Other(OtherKind::Sheathing))
                .cloned()
                .collect::<Vec<_>>(),
        )
        .unwrap();
        assert!(close(lo[0], 0.0) && close(hi[0], 216.0), "{lo:?} {hi:?}");
        assert!(close(lo[1], 0.0) && close(hi[1], 168.0), "{lo:?} {hi:?}");
        assert!(close(lo[2], 0.0) && close(hi[2], 120.0), "{lo:?} {hi:?}");

        // The left wall's studs stand inside the corners, within its depth.
        for s in m.iter().filter(|s| s.part == "left" && s.member.is_some()) {
            for c in corners_of(s) {
                assert!(c[0] > -0.01 && c[0] < 3.51, "{} at {c:?}", s.label);
                assert!(c[1] > 3.49 && c[1] < 164.51, "{} at {c:?}", s.label);
            }
        }
    }

    #[test]
    fn rafters_bear_on_both_cap_plates() {
        let (p, m) = shed();
        let f = frame_project(&p).unwrap();
        let back = f.walls[1].as_ref().unwrap().height.as_inches_f64();
        let rafter = m
            .iter()
            .find(|s| s.kind == SolidKind::Member(MemberKind::Rafter))
            .unwrap();
        // The front seat is level with the front wall's top, the back seat
        // with the back wall's.
        assert!(rafter.profile.contains(&[0.0, 120.0]));
        assert!(rafter.profile.contains(&[3.5, 120.0]));
        assert!(rafter
            .profile
            .iter()
            .any(|&[y, z]| close(y, 164.5) && close(z, back)));
        assert!(rafter
            .profile
            .iter()
            .any(|&[y, z]| close(y, 168.0) && close(z, back)));
    }

    #[test]
    fn porch_and_sheathing_are_in_the_model() {
        let (_, m) = shed();
        let part = |name: &str| -> Vec<Solid> {
            m.iter()
                .filter(|s| s.part == name && s.member.is_some())
                .cloned()
                .collect()
        };
        // The porch sits in front of the front wall, its roof under the
        // main roof's eave.
        let [lo, hi] = bounds(&part("porch")).unwrap();
        assert!(hi[1] <= 0.01 && lo[1] < -72.0, "{lo:?} {hi:?}");
        assert!(hi[2] <= 117.01, "{hi:?}");
        // The deck sits a step below the floor.
        let [_, hi] = bounds(&part("deck")).unwrap();
        assert!(close(hi[2], -1.0), "{hi:?}");
        // Wall sheathing hangs off the outside faces.
        let sheathing: Vec<Solid> = m
            .iter()
            .filter(|s| s.kind == SolidKind::Other(OtherKind::Sheathing) && s.part == "front")
            .cloned()
            .collect();
        let [lo, hi] = bounds(&sheathing).unwrap();
        assert!(
            close(lo[1], -7.0 / 16.0) && close(hi[1], 0.0),
            "{lo:?} {hi:?}"
        );
        // Roof sheathing lies on the rafters' top edges, above them.
        let roof: Vec<Solid> = m
            .iter()
            .filter(|s| s.kind == SolidKind::Other(OtherKind::Sheathing) && s.part == "roof")
            .cloned()
            .collect();
        let [lo, _] = bounds(&roof).unwrap();
        let rafters: Vec<Solid> = part("roof");
        let [rlo, _] = bounds(&rafters).unwrap();
        assert!(lo[2] > rlo[2], "{lo:?} {rlo:?}");
    }

    #[test]
    fn floor_hangs_below_the_walls() {
        let (_, m) = shed();
        let floor: Vec<Solid> = m.iter().filter(|s| s.part == "floor").cloned().collect();
        let [lo, hi] = bounds(&floor).unwrap();
        assert!(close(hi[2], 0.0), "{hi:?}");
        assert!(lo[2] < -(0.75 + 5.5 + 5.5));
    }

    #[test]
    fn free_walls_stand_in_a_row() {
        let p = Project::from_toml(
            "name = \"x\"\n[[wall]]\nid = \"A\"\nlength = \"8'\"\nheight = \"8'\"\n[[wall]]\nid = \"B\"\nlength = \"8'\"\nheight = \"8'\"\n",
        )
        .unwrap();
        let f = frame_project(&p).unwrap();
        let m = model(&p, &f);
        let [lo, hi] = bounds(&m).unwrap();
        assert!(close(lo[0], 0.0) && close(hi[0], 96.0 * 2.0 + 48.0));
        assert_eq!(wall_ids(&p), ["A", "B"]);
    }
}
