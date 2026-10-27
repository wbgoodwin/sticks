//! The plate layout: every line to square across a wall's bottom and top
//! plates, and which board goes beside it, before the wall goes together.
//!
//! Each mark is a board's near edge measured from the layout end, and the
//! board goes on the far side of the line. The plates are laid out side by
//! side, so a mark both plates share is made on both at once. On a rake
//! wall the top plate slopes, so its marks are measured along it and come
//! out a little longer than the bottom plate's.

use serde::Serialize;
use sticks_core::{LayoutStart, Length, MemberKind};

use crate::wall::FramedWall;

/// What gets marked on one wall's plates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PlateLayout {
    /// The top plate follows a slope, so its marks differ from the bottom plate's.
    pub top_sloped: bool,
    /// Lines in order from the layout end.
    pub marks: Vec<PlateMark>,
    /// Each opening's rough opening, measured like the marks.
    pub openings: Vec<PlateOpening>,
    /// Plate joints, each centered on a board.
    pub splices: Vec<PlateSplice>,
}

/// One line across the plates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PlateMark {
    /// The near edge from the layout end, along the bottom plate.
    pub at: Length,
    /// The same line along the top plate.
    pub top_at: Length,
    /// The board that stands on the bottom plate here.
    pub bottom: Option<PlateBoard>,
    /// The board that meets the top plate here.
    pub top: Option<PlateBoard>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PlateBoard {
    pub label: String,
    pub kind: MemberKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PlateOpening {
    pub name: String,
    /// The rough opening's near and far edges from the layout end.
    pub from: Length,
    pub to: Length,
    /// The bottom plate stops across it, as under a door.
    pub no_bottom_plate: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PlateSplice {
    pub kind: MemberKind,
    /// The joint from the layout end, along its own plate.
    pub at: Length,
}

/// The letter framers write beside a line for this board: X for a stud.
pub fn plate_letter(kind: MemberKind) -> &'static str {
    match kind {
        MemberKind::KingStud => "K",
        MemberKind::JackStud => "J",
        MemberKind::Cripple => "C",
        _ => "X",
    }
}

pub fn plate_layout(wall: &FramedWall, from: LayoutStart) -> PlateLayout {
    let len = wall.length;
    let rise = (wall.right_height - wall.left_height).as_inches_f64();
    let run = len.as_inches_f64().max(1.0);
    let top_sloped = wall.right_height != wall.left_height;
    let slope = (1.0 + (rise / run).powi(2)).sqrt();
    let along = |l: Length| {
        if top_sloped {
            Length::from_inches_f64(l.as_inches_f64() * slope)
        } else {
            l
        }
    };
    let from_end = |x: Length, w: Length| match from {
        LayoutStart::Left => x,
        LayoutStart::Right => len - x - w,
    };
    let bottom_plates: Vec<_> = wall
        .members
        .iter()
        .filter(|p| p.member.kind == MemberKind::BottomPlate)
        .collect();
    let plate_top = bottom_plates
        .first()
        .map(|p| p.rect.top())
        .unwrap_or_default();

    let mut marks: Vec<PlateMark> = Vec::new();
    for p in &wall.members {
        let kind = p.member.kind;
        let (bottom, top) = match kind {
            MemberKind::Stud | MemberKind::KingStud => (true, true),
            MemberKind::JackStud => (true, false),
            MemberKind::Cripple => (p.rect.y == plate_top, p.rect.y != plate_top),
            _ => continue,
        };
        let board = || PlateBoard {
            label: p.member.label.clone(),
            kind,
        };
        let at = from_end(p.rect.x, p.rect.w);
        // A cripple under a sill and one over the header share a line.
        let i = match marks.iter().position(|m| m.at == at) {
            Some(i) => i,
            None => {
                marks.push(PlateMark {
                    at,
                    top_at: along(at),
                    bottom: None,
                    top: None,
                });
                marks.len() - 1
            }
        };
        if bottom && marks[i].bottom.is_none() {
            marks[i].bottom = Some(board());
        }
        if top && marks[i].top.is_none() {
            marks[i].top = Some(board());
        }
    }
    marks.sort_by_key(|m| m.at);

    let openings = wall
        .openings
        .iter()
        .map(|o| {
            let (a, b) = (o.rect.x, o.rect.right());
            let (near, far) = match from {
                LayoutStart::Left => (a, b),
                LayoutStart::Right => (len - b, len - a),
            };
            PlateOpening {
                name: o.name.clone(),
                from: near,
                to: far,
                no_bottom_plate: !bottom_plates
                    .iter()
                    .any(|p| p.rect.x < b && p.rect.right() > a),
            }
        })
        .collect();

    let mut splices: Vec<PlateSplice> = wall
        .splices
        .iter()
        .map(|s| {
            let at = from_end(s.at, Length::ZERO);
            PlateSplice {
                kind: s.kind,
                at: if s.kind == MemberKind::BottomPlate {
                    at
                } else {
                    along(at)
                },
            }
        })
        .collect();
    splices.sort_by_key(|s| s.at);

    PlateLayout {
        top_sloped,
        marks,
        openings,
        splices,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame_project;
    use sticks_core::Project;

    fn shed() -> crate::FramedProject {
        let p = Project::from_toml(include_str!("../../../examples/shed.toml")).unwrap();
        frame_project(&p).unwrap()
    }

    fn wall<'a>(f: &'a crate::FramedProject, id: &str) -> &'a FramedWall {
        f.walls.iter().flatten().find(|w| w.id == id).expect("wall")
    }

    #[test]
    fn studs_mark_both_plates_and_jacks_only_the_bottom() {
        let f = shed();
        let w = wall(&f, "front");
        let layout = plate_layout(w, LayoutStart::Left);
        assert!(!layout.top_sloped);
        assert_eq!(layout.marks[0].at, Length::ZERO);
        let on_center = layout
            .marks
            .iter()
            .find(|m| m.at == Length::from_sixteenths(15 * 16 + 4))
            .expect("a stud at 15-1/4");
        assert_eq!(on_center.bottom.as_ref().unwrap().kind, MemberKind::Stud);
        assert_eq!(on_center.top_at, on_center.at);
        for m in &layout.marks {
            if let Some(b) = &m.bottom {
                if b.kind == MemberKind::JackStud {
                    assert!(m.top.is_none(), "a jack never reaches the top plate");
                }
            }
            assert!(m.bottom.is_some() || m.top.is_some());
        }
        assert!(layout
            .marks
            .iter()
            .any(|m| m.bottom.as_ref().map(|b| b.kind) == Some(MemberKind::JackStud)));
        // Every board standing in the wall is accounted for.
        let verticals = w
            .members
            .iter()
            .filter(|p| {
                matches!(
                    p.member.kind,
                    MemberKind::Stud
                        | MemberKind::KingStud
                        | MemberKind::JackStud
                        | MemberKind::Cripple
                )
            })
            .count();
        let listed = layout
            .marks
            .iter()
            .map(|m| {
                let both = m.bottom.as_ref().zip(m.top.as_ref());
                match both {
                    Some((b, t)) if b.label == t.label => 1,
                    Some(_) => 2,
                    None => 1,
                }
            })
            .sum::<usize>();
        assert_eq!(listed, verticals);
    }

    #[test]
    fn a_door_leaves_a_gap_in_the_bottom_plate() {
        let f = shed();
        let w = wall(&f, "front");
        let layout = plate_layout(w, LayoutStart::Left);
        let door = layout
            .openings
            .iter()
            .find(|o| o.name.starts_with("Door"))
            .expect("a door");
        assert!(door.no_bottom_plate);
        assert!(layout
            .openings
            .iter()
            .filter(|o| o.name.starts_with("Window"))
            .all(|o| !o.no_bottom_plate));
    }

    #[test]
    fn a_rake_walls_top_plate_is_measured_along_its_slope() {
        let f = shed();
        let w = wall(&f, "left");
        let layout = plate_layout(w, LayoutStart::Left);
        assert!(layout.top_sloped);
        let far = layout.marks.last().unwrap();
        assert!(far.top_at > far.at);
        // 2:12 runs about 1.4% longer along the slope.
        let stretch = far.top_at.as_inches_f64() / far.at.as_inches_f64();
        assert!(
            (stretch - (1.0f64 + 1.0 / 36.0).sqrt()).abs() < 0.002,
            "{stretch}"
        );
    }

    #[test]
    fn layout_from_the_right_counts_from_the_right_end() {
        let f = shed();
        let w = wall(&f, "front");
        let left = plate_layout(w, LayoutStart::Left);
        let right = plate_layout(w, LayoutStart::Right);
        assert_eq!(right.marks[0].at, Length::ZERO);
        assert_eq!(left.marks.len(), right.marks.len());
        let o = &right.openings[0];
        assert!(o.from < o.to);
    }
}
