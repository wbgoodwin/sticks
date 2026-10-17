//! A whole project: the building's four outside walls generated from its
//! footprint, any free-standing walls, the floor, the roof, the porch and
//! the sheathing.
//!
//! Seen from outside, each wall's left end is on your left. So the front
//! wall runs from the left side to the right side, the right wall from the
//! front to the back, the back wall from the right side to the left, and the
//! left wall from the back to the front. The front and back walls run
//! through the corners; the side walls fit between them.

use serde::Serialize;
use sticks_core::{
    LayoutStart, Length, Member, MemberKind, Project, RoofKind, SheetPiece, Wall, WallEnd,
};

use crate::floor::{frame_floor, FramedFloor};
use crate::layout::LayoutMark;
use crate::porch::{frame_porch, FramedPorch};
use crate::roof::{frame_roof, gable_studs, wall_heights, FramedRoof, RoofPlane};
use crate::sheathing::{frame_sheathing, FramedSheathing, WallFace};
use crate::wall::{frame_wall, frame_wall_shaped, FramedWall, WallShape};
use crate::FramingError;

/// Wall ids that name a side of the building, in drawing order.
pub const SIDES: [&str; 4] = ["front", "back", "left", "right"];

/// Everything framed from one project. Each part frames or fails on its own,
/// so one bad wall doesn't hide the rest.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FramedProject {
    /// Building walls first (front, back, left, right), then the others in
    /// the order the project lists them.
    pub walls: Vec<Result<FramedWall, FramingError>>,
    pub floor: Option<Result<FramedFloor, FramingError>>,
    pub roof: Option<Result<FramedRoof, FramingError>>,
    pub porch: Option<Result<FramedPorch, FramingError>>,
    pub sheathing: Option<Result<FramedSheathing, FramingError>>,
}

impl FramedProject {
    /// Every member of every part that framed.
    pub fn members(&self) -> Vec<Member> {
        let walls = self.walls.iter().flatten().flat_map(|w| &w.members);
        let floor = self.floor.iter().flatten().flat_map(|f| &f.members);
        let roof = self.roof.iter().flatten().flat_map(|r| &r.members);
        let porch = self.porch.iter().flatten().flat_map(|p| {
            p.members
                .iter()
                .chain(p.deck.iter().flat_map(|d| &d.members))
        });
        walls
            .chain(floor)
            .chain(roof)
            .chain(porch)
            .map(|p| p.member.clone())
            .collect()
    }

    /// Every piece of sheet goods: the subfloor and the sheathing.
    pub fn sheet_pieces(&self) -> Vec<SheetPiece> {
        let subfloor = self
            .floor
            .iter()
            .flatten()
            .flat_map(|f| &f.subfloor)
            .map(|s| &s.piece);
        let sheathing = self
            .sheathing
            .iter()
            .flatten()
            .flat_map(|s| s.panels())
            .map(|p| &p.piece);
        subfloor.chain(sheathing).cloned().collect()
    }

    /// The parts that didn't frame.
    pub fn errors(&self) -> Vec<&FramingError> {
        let walls = self.walls.iter().filter_map(|w| w.as_ref().err());
        let floor = self.floor.iter().filter_map(|f| f.as_ref().err());
        let roof = self.roof.iter().filter_map(|r| r.as_ref().err());
        let porch = self.porch.iter().filter_map(|p| p.as_ref().err());
        let sheathing = self.sheathing.iter().filter_map(|s| s.as_ref().err());
        walls
            .chain(floor)
            .chain(roof)
            .chain(porch)
            .chain(sheathing)
            .collect()
    }
}

/// Frames every part of `project`. Fails outright only when the project
/// itself doesn't hang together, such as a roof with no building under it.
pub fn frame_project(project: &Project) -> Result<FramedProject, FramingError> {
    let rules = &project.rules;
    let stock = &project.stock;
    let Some(building) = &project.building else {
        for (part, given) in [
            ("floor", project.floor.is_some()),
            ("roof", project.roof.is_some()),
            ("porch", project.porch.is_some()),
            ("sheathing", project.sheathing.is_some()),
        ] {
            if given {
                return Err(FramingError {
                    part: part.to_string(),
                    reason: "needs a [building] with a width, depth and height".to_string(),
                });
            }
        }
        let walls = project
            .walls
            .iter()
            .map(|w| frame_wall(w, rules, stock))
            .collect();
        return Ok(FramedProject {
            walls,
            floor: None,
            roof: None,
            porch: None,
            sheathing: None,
        });
    };

    let fail = |part: &str, reason: String| FramingError {
        part: part.to_string(),
        reason,
    };
    let depth = rules.wall_lumber.actual_width();
    if building.width <= depth * 2 || building.depth <= depth * 2 {
        return Err(fail(
            "building",
            format!(
                "{} by {} is too small to frame",
                building.width, building.depth
            ),
        ));
    }
    for wall in project
        .walls
        .iter()
        .filter(|w| SIDES.contains(&w.id.as_str()))
    {
        let given: Vec<&str> = [
            ("length", wall.length.is_some()),
            ("height", wall.height.is_some()),
            ("left_end", wall.left_end.is_some()),
            ("right_end", wall.right_end.is_some()),
        ]
        .into_iter()
        .filter_map(|(name, set)| set.then_some(name))
        .collect();
        if !given.is_empty() {
            return Err(fail(
                &format!("wall {}", wall.id),
                format!(
                    "takes its {} from the building; leave {} out",
                    given.join(" and "),
                    if given.len() == 1 { "it" } else { "them" }
                ),
            ));
        }
    }

    // Under a lean-to the back wall drops to meet the slope and the side walls
    // rake; under a gable every wall is level.
    let (front, back, side_front, side_back) = match &project.roof {
        Some(roof) => {
            let h = wall_heights(building, roof, depth);
            let shortest = rules.wall_lumber.actual_thickness() * 3 + Length::inches(12);
            if h.back < shortest {
                return Err(fail(
                    "roof",
                    format!(
                        "at {} the back wall would be only {} tall; lower the pitch or raise the front wall",
                        roof.pitch, h.back
                    ),
                ));
            }
            (h.front, h.back, h.side_front, h.side_back)
        }
        None => {
            let h = building.height;
            (h, h, h, h)
        }
    };
    let side_len = building.depth - depth * 2;
    let shapes = [
        (
            building.width,
            front,
            front,
            WallEnd::Corner,
            WallEnd::Corner,
        ),
        (building.width, back, back, WallEnd::Corner, WallEnd::Corner),
        (
            side_len,
            side_back,
            side_front,
            WallEnd::Plain,
            WallEnd::Plain,
        ),
        (
            side_len,
            side_front,
            side_back,
            WallEnd::Plain,
            WallEnd::Plain,
        ),
    ];

    let mut walls = Vec::new();
    for (id, (length, left_height, right_height, left_end, right_end)) in SIDES.iter().zip(shapes) {
        let wall = project
            .walls
            .iter()
            .find(|w| w.id == *id)
            .cloned()
            .unwrap_or_else(|| Wall {
                id: id.to_string(),
                length: None,
                height: None,
                left_end: None,
                right_end: None,
                tees: Vec::new(),
                openings: Vec::new(),
            });
        let shape = WallShape {
            length,
            left_height,
            right_height,
            left_end,
            right_end,
        };
        walls.push(frame_wall_shaped(&wall, &shape, rules, stock));
    }
    walls.extend(
        project
            .walls
            .iter()
            .filter(|w| !SIDES.contains(&w.id.as_str()))
            .map(|w| frame_wall(w, rules, stock)),
    );

    let floor = project
        .floor
        .as_ref()
        .map(|f| frame_floor(f, building, rules, stock, &project.sheets));
    let roof = project
        .roof
        .as_ref()
        .map(|r| frame_roof(r, building, rules, stock));

    // Gable ends: studs from the side walls' cap plates up to the end rafters.
    if let Some(Ok(roof)) = &roof {
        if roof.kind == RoofKind::Gable {
            for wall in walls[2..4].iter_mut().flatten() {
                let plan_y = side_plan_y(&wall.id, building.depth, depth);
                let studs: Vec<Length> = wall
                    .members
                    .iter()
                    .filter(|p| p.member.kind == MemberKind::Stud)
                    .map(|p| p.rect.x)
                    .collect();
                let gable = gable_studs(
                    roof,
                    &wall.id,
                    wall.height,
                    &studs,
                    rules.wall_lumber,
                    plan_y,
                );
                for p in &gable {
                    wall.height = wall.height.max(p.rect.top());
                    wall.marks.push(LayoutMark {
                        label: p.member.label.clone(),
                        kind: MemberKind::GableStud,
                        at: match rules.layout_from {
                            LayoutStart::Left => p.rect.x,
                            LayoutStart::Right => wall.length - p.rect.right(),
                        },
                    });
                }
                wall.marks.sort_by_key(|m| m.at);
                wall.marks.dedup_by_key(|m| m.at);
                wall.members.extend(gable);
            }
        }
    }

    // Wall sheathing runs down over the floor's rim.
    let rim_bottom = match (&floor, &project.floor) {
        (Some(Ok(_)), Some(f)) => -(f.subfloor + f.joist.actual_width()),
        _ => Length::ZERO,
    };
    let porch = project.porch.as_ref().map(|p| {
        frame_porch(
            p,
            building,
            project.roof.as_ref(),
            project.floor.as_ref(),
            rules,
            stock,
        )
    });

    let sheathing = project.sheathing.as_ref().map(|s| {
        let planes: Vec<&RoofPlane> = match &roof {
            Some(Ok(r)) => r.planes.iter().collect(),
            _ => Vec::new(),
        };
        let faces: Vec<WallFace> = walls[..4]
            .iter()
            .flatten()
            .map(|wall| wall_face(wall, building.depth, depth, &planes))
            .collect();
        frame_sheathing(
            s,
            &faces,
            roof.as_ref().and_then(|r| r.as_ref().ok()),
            porch.as_ref().and_then(|p| p.as_ref().ok()),
            rim_bottom,
            rules,
            &project.sheets,
        )
    });
    Ok(FramedProject {
        walls,
        floor,
        roof,
        porch,
        sheathing,
    })
}

/// Turns a spot along a side wall (from its left end, seen from outside)
/// into plan `y`. The left wall's left end is at the back.
pub fn side_plan_y(
    id: &str,
    building_depth: Length,
    wall_depth: Length,
) -> impl Fn(Length) -> Length {
    let left = id == "left";
    move |x| {
        if left {
            building_depth - wall_depth - x
        } else {
            wall_depth + x
        }
    }
}

/// Height of `plane`'s top surface over plan `y`, when `y` is under it.
fn plane_height(plane: &RoofPlane, y: Length) -> Option<Length> {
    let (ey, ez) = plane.eave;
    let (ty, tz) = plane.top;
    let (lo, hi) = (ey.min(ty), ey.max(ty));
    if y < lo || y > hi || ty == ey {
        return None;
    }
    Some(ez + (tz - ez).mul_div((y - ey).sixteenths(), (ty - ey).sixteenths()))
}

/// A building wall's outside face for sheathing: the front and back walls'
/// own length, the side walls' plus the ends of the front and back walls,
/// up to the cap plate or, on the sides, the top of the end rafters.
fn wall_face<'a>(
    wall: &'a FramedWall,
    building_depth: Length,
    wall_depth: Length,
    planes: &[&RoofPlane],
) -> WallFace<'a> {
    let side = wall.id == "left" || wall.id == "right";
    if !side || planes.is_empty() {
        let (from, to) = if side {
            (-wall_depth, wall.length + wall_depth)
        } else {
            (Length::ZERO, wall.length)
        };
        return WallFace {
            wall,
            from,
            to,
            top: vec![(from, wall.left_height), (to, wall.right_height)],
        };
    }
    let plan_y = side_plan_y(&wall.id, building_depth, wall_depth);
    let (from, to) = (-wall_depth, wall.length + wall_depth);
    let height = |x: Length| {
        let y = plan_y(x);
        planes
            .iter()
            .filter_map(|p| plane_height(p, y))
            .max()
            .unwrap_or(wall.left_height)
    };
    let mut xs = vec![from, to];
    // A gable's peak, where the two slopes meet over the wall.
    for p in planes {
        let x = if wall.id == "left" {
            building_depth - wall_depth - p.top.0
        } else {
            p.top.0 - wall_depth
        };
        if x > from && x < to {
            xs.push(x);
        }
    }
    xs.sort();
    xs.dedup();
    WallFace {
        wall,
        from,
        to,
        top: xs.into_iter().map(|x| (x, height(x))).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(s: &str) -> Length {
        s.parse().unwrap()
    }

    #[test]
    fn frames_the_reference_shed() {
        let p = Project::from_toml(include_str!("../../../examples/shed.toml")).unwrap();
        let f = frame_project(&p).unwrap();
        assert!(f.errors().is_empty(), "{:?}", f.errors());
        let walls: Vec<&FramedWall> = f.walls.iter().flatten().collect();
        let ids: Vec<_> = walls.iter().map(|w| w.id.as_str()).collect();
        assert_eq!(ids, SIDES);

        let (front, back, left, right) = (walls[0], walls[1], walls[2], walls[3]);
        assert_eq!(front.length, l("18'"));
        assert_eq!(front.height, l("10'"));
        // 2:12 drops 27-7/16" over the 164-1/2" from the front wall's
        // outside face to the back wall's inside face.
        assert_eq!(back.height, l("92-9/16"));
        // Side walls fit between the front and back walls and rake down to the back.
        assert_eq!(left.length, l("14'") - l("7"));
        assert_eq!(
            (left.left_height, left.right_height),
            (back.height, l("119-7/16"))
        );
        assert_eq!(
            (right.left_height, right.right_height),
            (l("119-7/16"), back.height)
        );

        // No piece is longer than the stock it's cut from.
        let members = f.members();
        for m in &members {
            let longest = sticks_core::longest_board(&p.stock, m.size, p.rules.end_trim);
            assert!(m.length <= longest, "{} is {}", m.label, m.length);
        }
        assert!(members.iter().any(|m| m.kind == MemberKind::Rafter));
        assert!(members.iter().any(|m| m.kind == MemberKind::Joist));
        assert!(!f.sheet_pieces().is_empty());
        assert!(f.porch.as_ref().unwrap().is_ok());
        let sheathing = f.sheathing.as_ref().unwrap().as_ref().unwrap();
        assert_eq!(sheathing.walls.len(), 4);
        // The lean-to and the porch roof.
        assert_eq!(sheathing.roofs.len(), 2);
        assert!(sheathing.warnings.is_empty(), "{:?}", sheathing.warnings);
    }

    #[test]
    fn wall_sheathing_joints_land_on_studs() {
        let p = Project::from_toml(include_str!("../../../examples/shed.toml")).unwrap();
        let f = frame_project(&p).unwrap();
        let s = f.sheathing.as_ref().unwrap().as_ref().unwrap();
        for (face, wall) in s.walls.iter().zip(f.walls.iter().flatten()) {
            let studs: Vec<(Length, Length)> = wall
                .members
                .iter()
                .filter(|m| matches!(m.member.kind, MemberKind::Stud | MemberKind::KingStud))
                .map(|m| (m.rect.x, m.rect.right()))
                .collect();
            let first = face.outline[0].0;
            let last = face.outline[1].0;
            for panel in &face.panels {
                assert!(panel.rect.w <= l("48") && panel.rect.h <= l("96"));
                for x in [panel.rect.x, panel.rect.right()] {
                    if x == first || x == last {
                        continue;
                    }
                    assert!(
                        studs.iter().any(|&(a, b)| x > a && x < b),
                        "{} edge at {x:#} misses the studs",
                        panel.piece.label
                    );
                }
            }
        }
        // The side walls cover the front and back walls' ends.
        let left = &s.walls[2];
        assert_eq!(left.outline[0].0, -l("3-1/2"));
        // The door is cut out of the front wall's sheathing.
        assert!(s.walls[0].panels.iter().any(|p| !p.holes.is_empty()));
        // Every piece fits a 4x8 sheet.
        for p in s.panels() {
            assert!(
                p.piece.length <= l("8'") && p.piece.width <= l("4'"),
                "{}",
                p.piece.label
            );
        }
    }

    #[test]
    fn gable_roof_frames_level_walls_and_gable_studs() {
        let toml = include_str!("../../../examples/shed.toml")
            .replace("pitch = \"2:12\"", "kind = \"gable\"\npitch = \"6:12\"");
        let p = Project::from_toml(&toml).unwrap();
        let f = frame_project(&p).unwrap();
        assert!(f.errors().is_empty(), "{:?}", f.errors());
        let walls: Vec<&FramedWall> = f.walls.iter().flatten().collect();
        assert_eq!(walls[1].height, l("10'"));
        let roof = f.roof.as_ref().unwrap().as_ref().unwrap();
        let ridge = roof.ridge.unwrap();
        // The ridge board is centered front to back.
        assert_eq!(ridge.y + ridge.thickness.mul_div(1, 2), l("7'"));
        let count = |kind| {
            roof.members
                .iter()
                .filter(|m| m.member.kind == kind)
                .count()
        };
        assert_eq!(count(MemberKind::Rafter), 30);
        // 18' of ridge from 16' stock (2x8 isn't listed): two pieces.
        assert_eq!(count(MemberKind::Ridge), 2);
        assert_eq!(count(MemberKind::CollarTie), 4);
        let d = &roof.rafters[0];
        assert_eq!((d.count, d.heels.len()), (30, 1));
        // 6:12 over 84 - 3/4 = 83-1/4" of reach and a 12" tail.
        assert_eq!(d.run, l("95-1/4"));

        // Gable studs over the left wall's studs, tallest under the ridge.
        let left = walls[2];
        let gable: Vec<_> = left
            .members
            .iter()
            .filter(|m| m.member.kind == MemberKind::GableStud)
            .collect();
        assert!(gable.len() > 6);
        let tallest = gable.iter().map(|g| g.rect.top()).max().unwrap();
        assert_eq!(tallest, ridge.bottom);
        assert_eq!(left.height, ridge.bottom);
        // The roof sheathing has a front and a back slope, and the side
        // walls' sheathing peaks under the ridge.
        let s = f.sheathing.as_ref().unwrap().as_ref().unwrap();
        assert_eq!(s.roofs.len(), 3);
        assert_eq!(s.walls[2].outline.len(), 5);
    }

    #[test]
    fn building_walls_take_their_size_from_the_building() {
        let p = Project::from_toml(
            "name = \"x\"\n[building]\nwidth = \"10'\"\ndepth = \"8'\"\nheight = \"8'\"\n[[wall]]\nid = \"front\"\nlength = \"10'\"\n",
        )
        .unwrap();
        let err = frame_project(&p).unwrap_err().to_string();
        assert_eq!(
            err,
            "wall front: takes its length from the building; leave it out"
        );
    }

    #[test]
    fn a_roof_needs_a_building() {
        let p = Project::from_toml("name = \"x\"\n[roof]\npitch = \"3:12\"\n").unwrap();
        assert!(frame_project(&p)
            .unwrap_err()
            .to_string()
            .starts_with("roof: needs a [building]"));
    }

    #[test]
    fn a_steep_roof_explains_the_short_back_wall() {
        let p = Project::from_toml(
            "name = \"x\"\n[building]\nwidth = \"10'\"\ndepth = \"14'\"\nheight = \"8'\"\n[roof]\npitch = \"8:12\"\n",
        )
        .unwrap();
        let err = frame_project(&p).unwrap_err().to_string();
        assert!(err.contains("lower the pitch"), "{err}");
    }

    #[test]
    fn projects_without_a_building_frame_their_walls() {
        let p = Project::from_toml(
            "name = \"x\"\n[[wall]]\nid = \"front\"\nlength = \"8'\"\nheight = \"8'\"\n",
        )
        .unwrap();
        let f = frame_project(&p).unwrap();
        assert_eq!(f.walls.len(), 1);
        assert!(f.floor.is_none());
    }
}
