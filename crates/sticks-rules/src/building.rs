//! A whole project: the building's four outside walls generated from its
//! footprint, any free-standing walls, the floor and the roof.
//!
//! Seen from outside, each wall's left end is on your left. So the front
//! wall runs from the left side to the right side, the right wall from the
//! front to the back, the back wall from the right side to the left, and the
//! left wall from the back to the front. The front and back walls run
//! through the corners; the side walls fit between them.

use serde::Serialize;
use sticks_core::{Length, Member, Project, SheetPiece, Wall, WallEnd};

use crate::floor::{frame_floor, FramedFloor};
use crate::roof::{frame_roof, lean_to_heights, FramedRoof};
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
}

impl FramedProject {
    /// Every member of every part that framed.
    pub fn members(&self) -> Vec<Member> {
        let walls = self.walls.iter().flatten().flat_map(|w| &w.members);
        let floor = self.floor.iter().flatten().flat_map(|f| &f.members);
        let roof = self.roof.iter().flatten().flat_map(|r| &r.members);
        walls
            .chain(floor)
            .chain(roof)
            .map(|p| p.member.clone())
            .collect()
    }

    /// Every piece of sheet goods, such as the subfloor.
    pub fn sheet_pieces(&self) -> Vec<SheetPiece> {
        self.floor
            .iter()
            .flatten()
            .flat_map(|f| &f.subfloor)
            .map(|s| s.piece.clone())
            .collect()
    }

    /// The parts that didn't frame.
    pub fn errors(&self) -> Vec<&FramingError> {
        let walls = self.walls.iter().filter_map(|w| w.as_ref().err());
        let floor = self.floor.iter().filter_map(|f| f.as_ref().err());
        let roof = self.roof.iter().filter_map(|r| r.as_ref().err());
        walls.chain(floor).chain(roof).collect()
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

    // Under a lean-to the back wall drops to meet the slope and the side walls rake.
    let (front, back, side_front, side_back) = match &project.roof {
        Some(roof) => {
            let h = lean_to_heights(building, roof.pitch, depth);
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
    Ok(FramedProject { walls, floor, roof })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sticks_core::MemberKind;

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
        assert_eq!(back.height, l("78-7/8"));
        // Side walls fit between the front and back walls and rake down to the back.
        assert_eq!(left.length, l("14'") - l("7"));
        assert_eq!(
            (left.left_height, left.right_height),
            (back.height, l("119-1/8"))
        );
        assert_eq!(
            (right.left_height, right.right_height),
            (l("119-1/8"), back.height)
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
