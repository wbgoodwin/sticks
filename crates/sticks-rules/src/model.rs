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
//! birdsmouths included, pushed across its thickness.

use serde::Serialize;
use sticks_core::{Length, Member, MemberKind, Project};

use crate::building::{FramedProject, SIDES};
use crate::floor::FramedFloor;
use crate::layout::{Placed, Rect};
use crate::roof::FramedRoof;
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
}

/// One piece of the model.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Solid {
    pub label: String,
    /// The part it belongs to: a wall id, `floor` or `roof`.
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

/// Every member of `framed` placed in the building, plus the piers and
/// subfloor. Walls that aren't part of a building are stood in a row along
/// the front line when the project has no building, and left out otherwise,
/// since nothing says where they go.
pub fn model(project: &Project, framed: &FramedProject) -> Vec<Solid> {
    let wall_depth = inches(project.rules.wall_lumber.actual_width());
    let mut out = Vec::new();
    match &project.building {
        Some(b) => {
            let (w, d) = (inches(b.width), inches(b.depth));
            for wall in framed.walls.iter().flatten() {
                // Origin on the inside face at the wall's left end; u along
                // the wall as seen from outside, so u × v points outside.
                let (origin, u) = match wall.id.as_str() {
                    "front" => ([0.0, wall_depth, 0.0], [1.0, 0.0, 0.0]),
                    "back" => ([w, d - wall_depth, 0.0], [-1.0, 0.0, 0.0]),
                    "left" => ([wall_depth, d - wall_depth, 0.0], [0.0, -1.0, 0.0]),
                    "right" => ([w - wall_depth, wall_depth, 0.0], [0.0, 1.0, 0.0]),
                    _ => continue,
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
    out
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
/// the joists, piers under the beams.
fn floor_solids(out: &mut Vec<Solid>, floor: &FramedFloor, subfloor: f64, joist: f64) {
    let joist_bottom = -subfloor - joist;
    let mut beam_bottom = joist_bottom;
    for p in &floor.members {
        let height = inches(p.member.size.actual_width());
        let bottom = match p.member.kind {
            MemberKind::Beam | MemberKind::Skid => {
                beam_bottom = beam_bottom.min(joist_bottom - height);
                joist_bottom - height
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
    const PIER: f64 = 12.0;
    const PIER_DROP: f64 = 16.0;
    for (i, pier) in floor.piers.iter().enumerate() {
        let (x, y) = (inches(pier.x) - PIER / 2.0, inches(pier.y) - PIER / 2.0);
        out.push(Solid {
            label: format!("pier {}", i + 1),
            part: "floor".to_string(),
            kind: SolidKind::Other(OtherKind::Pier),
            member: None,
            profile: vec![[x, y], [x + PIER, y], [x + PIER, y + PIER], [x, y + PIER]],
            origin: [0.0, 0.0, beam_bottom - PIER_DROP],
            u: [1.0, 0.0, 0.0],
            v: [0.0, 1.0, 0.0],
            depth: PIER_DROP,
        });
    }
}

/// Rafters as side profiles with their birdsmouths, sloping down from the
/// front wall to the back; fascia across the tails, flush with their tops.
fn roof_solids(out: &mut Vec<Solid>, roof: &FramedRoof, wall_depth: f64) {
    let r = &roof.rafter;
    let slope = inches(r.pitch.rise) / 12.0;
    let front = inches(roof.heights.front);
    let back = inches(roof.heights.back);
    let depth = inches(roof.depth);
    let (fo, bo) = (inches(r.front_overhang), inches(r.back_overhang));
    let plumb = inches(r.plumb_depth);
    // The rafter's bottom edge, which bears on the front wall's outside face.
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
        let x = inches(p.rect.x);
        match p.member.kind {
            MemberKind::Rafter => out.push(Solid {
                profile: profile.clone(),
                origin: [x, 0.0, 0.0],
                u: [0.0, 1.0, 0.0],
                v: UP,
                depth: inches(p.rect.w),
                ..member_solid(p, "roof")
            }),
            _ => {
                // Fascia: the front board's inside face is at the front tail.
                let top = if inches(p.rect.y) < 0.0 {
                    bottom(tail) + plumb
                } else {
                    bottom(end) + plumb
                };
                let height = inches(p.member.size.actual_width());
                out.push(Solid {
                    profile: corners(&p.rect),
                    origin: [0.0, 0.0, top - height],
                    depth: height,
                    ..member_solid(p, "roof")
                });
            }
        }
    }
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
                .filter(|s| s.part != "roof" && s.part != "floor")
                .cloned()
                .collect::<Vec<_>>(),
        )
        .unwrap();
        assert!(close(lo[0], 0.0) && close(hi[0], 216.0), "{lo:?} {hi:?}");
        assert!(close(lo[1], 0.0) && close(hi[1], 168.0), "{lo:?} {hi:?}");
        assert!(close(lo[2], 0.0) && close(hi[2], 120.0), "{lo:?} {hi:?}");

        // The left wall's studs stand inside the corners, within its depth.
        for s in m.iter().filter(|s| s.part == "left") {
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
