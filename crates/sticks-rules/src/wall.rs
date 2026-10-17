//! Stud walls: plates, layout studs, corners, tees and openings.
//!
//! Coordinates are the wall's elevation seen from outside: `x` runs from the
//! wall's left end, `y` from the bottom of the bottom plate.

use serde::Serialize;
use sticks_core::{
    LayoutStart, Length, Member, MemberKind, Nominal, Opening, OpeningKind, Rules, Wall, WallEnd,
};

/// Spacer blocks in a three-stud corner: how many, and how long each is.
const CORNER_BLOCKS: i64 = 3;
const CORNER_BLOCK_LENGTH: Length = Length::inches(12);
/// Cripples shorter than this are hard to nail without splitting.
const SHORT_CRIPPLE: Length = Length::inches(3);

/// An axis-aligned rectangle in a wall elevation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Rect {
    pub x: Length,
    pub y: Length,
    pub w: Length,
    pub h: Length,
}

impl Rect {
    fn right(&self) -> Length {
        self.x + self.w
    }
}

/// A member and where it sits in the elevation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Placed {
    pub member: Member,
    pub rect: Rect,
}

/// A measurement for build day: where a vertical member's near edge lands,
/// measured from the end the layout starts at.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LayoutMark {
    pub label: String,
    pub kind: MemberKind,
    pub at: Length,
}

/// Everything generated for one wall.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FramedWall {
    pub id: String,
    pub length: Length,
    pub height: Length,
    pub members: Vec<Placed>,
    /// Vertical members, ordered from the layout end.
    pub marks: Vec<LayoutMark>,
    /// Things that frame but deserve a second look.
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("wall {wall}: {reason}")]
pub struct FramingError {
    pub wall: String,
    pub reason: String,
}

/// A vertical member before it gets a label.
struct Vertical {
    kind: MemberKind,
    x: Length,
    y: Length,
    h: Length,
}

/// A horizontal member before it gets a label.
struct Horizontal {
    kind: MemberKind,
    size: Nominal,
    rect: Rect,
    /// Which ply of a built-up header this is, from 0.
    ply: u8,
}

/// An opening once its framing mode is decided.
struct Framed<'a> {
    index: usize,
    opening: &'a Opening,
    /// Small penetrations that miss every stud get blocking instead of a header.
    blocked: bool,
}

pub fn frame_wall(wall: &Wall, rules: &Rules) -> Result<FramedWall, FramingError> {
    let fail = |reason: String| FramingError {
        wall: wall.id.clone(),
        reason,
    };
    let lumber = rules.wall_lumber;
    let t = lumber.actual_thickness();
    let depth = lumber.actual_width();
    let len = wall.length;
    let height = wall.height;
    let stud_len = height - t * 3;
    let plate_top = t;
    let top_plate_bottom = height - t * 2;

    if rules.on_center < t * 2 {
        return Err(fail(format!(
            "studs {:#} on center leaves no room between studs",
            rules.on_center
        )));
    }
    if stud_len <= Length::ZERO {
        return Err(fail(format!(
            "{height} is too short for three {} plates",
            lumber
        )));
    }
    let left_reserve = end_reserve(wall.left_end, t);
    let right_reserve = end_reserve(wall.right_end, t);
    if len < left_reserve + right_reserve {
        return Err(fail(format!("{len} is too short for its end studs")));
    }

    // Zones that layout studs must stay out of, as [start, end) on x.
    let mut zones = vec![(Length::ZERO, left_reserve), (len - right_reserve, len)];
    let mut verticals = vec![
        Vertical {
            kind: MemberKind::Stud,
            x: Length::ZERO,
            y: plate_top,
            h: stud_len,
        },
        Vertical {
            kind: MemberKind::Stud,
            x: len - t,
            y: plate_top,
            h: stud_len,
        },
    ];
    let mut horizontals = Vec::new();
    for (end, block_x, backer_x) in [
        (wall.left_end, t, t * 2),
        (wall.right_end, len - t * 2, len - t * 3),
    ] {
        if end == WallEnd::Corner {
            verticals.push(Vertical {
                kind: MemberKind::Stud,
                x: backer_x,
                y: plate_top,
                h: stud_len,
            });
            for y in spread(plate_top, stud_len, CORNER_BLOCK_LENGTH, CORNER_BLOCKS) {
                verticals.push(Vertical {
                    kind: MemberKind::Blocking,
                    x: block_x,
                    y,
                    h: CORNER_BLOCK_LENGTH,
                });
            }
        }
    }

    for (i, &center) in wall.tees.iter().enumerate() {
        let a = center - half(depth) - t;
        let b = center + half(depth) + t;
        if a < left_reserve || b > len - right_reserve {
            return Err(fail(format!(
                "the tee at {center} is too close to an end of the wall"
            )));
        }
        if let Some(o) = wall
            .openings
            .iter()
            .find(|o| overlaps((a, b), (o.offset - t * 2, o.offset + o.width + t * 2)))
        {
            return Err(fail(format!(
                "the tee at {center} runs into the {} at {}",
                kind_name(o.kind),
                o.offset
            )));
        }
        if wall.tees[..i]
            .iter()
            .any(|&c| overlaps((a, b), (c - half(depth) - t, c + half(depth) + t)))
        {
            return Err(fail(format!("the tee at {center} overlaps another tee")));
        }
        zones.push((a, b));
        for x in [a, center + half(depth)] {
            verticals.push(Vertical {
                kind: MemberKind::Stud,
                x,
                y: plate_top,
                h: stud_len,
            });
        }
    }

    // Validate every opening's position and height before choosing how to frame it.
    let mut order: Vec<usize> = (0..wall.openings.len()).collect();
    order.sort_by_key(|&i| wall.openings[i].offset);
    for &i in &order {
        let o = &wall.openings[i];
        let name = || format!("{} {} (at {})", kind_name(o.kind), i + 1, o.offset);
        if o.width <= Length::ZERO || o.height <= Length::ZERO {
            return Err(fail(format!("{} needs a width and height", name())));
        }
        if o.offset < Length::ZERO || o.offset + o.width > len {
            return Err(fail(format!("{} doesn't fit inside the wall", name())));
        }
        if o.sill_height > Length::ZERO && o.sill_height < t * 2 + Length::from_sixteenths(1) {
            return Err(fail(format!(
                "{} sits too low: a sill needs more than {:#} above the floor",
                name(),
                t * 2
            )));
        }
    }

    // Penetrations that dodge every stud get blocking; the rest get full framing.
    let layout = layout_positions(len, rules, t);
    let mut framed: Vec<Framed> = order
        .iter()
        .map(|&i| Framed {
            index: i,
            opening: &wall.openings[i],
            blocked: wall.openings[i].kind == OpeningKind::Penetration,
        })
        .collect();
    loop {
        let fixed = fixed_zones(&zones, &framed, t);
        let mut changed = false;
        for f in framed.iter_mut().filter(|f| f.blocked) {
            let hole = (f.opening.offset, f.opening.offset + f.opening.width);
            let hits_layout = layout
                .iter()
                .filter(|&&x| !fixed.iter().any(|&z| overlaps((x, x + t), z)))
                .any(|&x| overlaps((x, x + t), hole));
            let hits_fixed = fixed.iter().any(|&z| overlaps(hole, z));
            if hits_layout || hits_fixed {
                f.blocked = false;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    // Openings with full framing need room for kings and must not collide.
    let mut last_end: Option<(Length, usize)> = None;
    for f in framed.iter().filter(|f| !f.blocked) {
        let o = f.opening;
        let a = o.offset - t * 2;
        let b = o.offset + o.width + t * 2;
        let name = format!("{} {}", kind_name(o.kind), f.index + 1);
        if a < left_reserve {
            return Err(fail(format!(
                "{name} is too close to the left end: its king and jack studs need {:#} from the end",
                left_reserve + t * 2
            )));
        }
        if b > len - right_reserve {
            return Err(fail(format!(
                "{name} is too close to the right end: its king and jack studs need {:#} from the end",
                right_reserve + t * 2
            )));
        }
        if let Some((end, j)) = last_end {
            if a < end {
                return Err(fail(format!(
                    "{name} overlaps opening {} once king and jack studs are added",
                    j + 1
                )));
            }
        }
        last_end = Some((b, f.index));

        let head_y = o.sill_height + o.height;
        let header_top = head_y + o.header.actual_width();
        if header_top > top_plate_bottom {
            return Err(fail(format!(
                "{name} is too tall: its {} header would end at {:#}, but the top plates start at {:#}",
                o.header, header_top, top_plate_bottom
            )));
        }
    }

    let fixed = fixed_zones(&zones, &framed, t);
    let mut warnings = Vec::new();
    for &x in &layout {
        if !fixed.iter().any(|&z| overlaps((x, x + t), z)) {
            verticals.push(Vertical {
                kind: MemberKind::Stud,
                x,
                y: plate_top,
                h: stud_len,
            });
        }
    }

    for f in framed.iter().filter(|f| !f.blocked) {
        let o = f.opening;
        let left = o.offset;
        let right = o.offset + o.width;
        let head_y = o.sill_height + o.height;
        let header_top = head_y + o.header.actual_width();
        let name = format!("{} {}", kind_name(o.kind), f.index + 1);

        for x in [left - t * 2, right + t] {
            verticals.push(Vertical {
                kind: MemberKind::KingStud,
                x,
                y: plate_top,
                h: stud_len,
            });
        }
        for x in [left - t, right] {
            verticals.push(Vertical {
                kind: MemberKind::JackStud,
                x,
                y: plate_top,
                h: head_y - plate_top,
            });
        }

        let header = Rect {
            x: left - t,
            y: head_y,
            w: o.width + t * 2,
            h: o.header.actual_width(),
        };
        for ply in 0..header_plies(depth, o.header.actual_thickness()) {
            horizontals.push(Horizontal {
                kind: MemberKind::Header,
                size: o.header,
                rect: header,
                ply,
            });
        }

        let top_gap = top_plate_bottom - header_top;
        let top_columns: Vec<Length> = layout
            .iter()
            .copied()
            .filter(|&x| x >= header.x && x + t <= header.right())
            .collect();
        if top_gap > Length::ZERO && !top_columns.is_empty() {
            if top_gap < SHORT_CRIPPLE {
                warnings.push(format!(
                    "{name}: cripples above the header are only {:#} long; a deeper header would fill the gap",
                    top_gap
                ));
            }
            for x in top_columns {
                verticals.push(Vertical {
                    kind: MemberKind::Cripple,
                    x,
                    y: header_top,
                    h: top_gap,
                });
            }
        }

        if o.sill_height > Length::ZERO {
            let sill_y = o.sill_height - t;
            horizontals.push(Horizontal {
                kind: MemberKind::Sill,
                size: lumber,
                rect: Rect {
                    x: left,
                    y: sill_y,
                    w: o.width,
                    h: t,
                },
                ply: 0,
            });
            let gap = sill_y - plate_top;
            let columns: Vec<Length> = layout
                .iter()
                .copied()
                .filter(|&x| x >= left && x + t <= right)
                .collect();
            if gap < SHORT_CRIPPLE && !columns.is_empty() {
                warnings.push(format!(
                    "{name}: cripples under the sill are only {:#} long",
                    gap
                ));
            }
            for x in columns {
                verticals.push(Vertical {
                    kind: MemberKind::Cripple,
                    x,
                    y: plate_top,
                    h: gap,
                });
            }
        }
    }

    for f in framed.iter().filter(|f| f.blocked) {
        let o = f.opening;
        let left_stud = verticals
            .iter()
            .filter(|v| full_height(v.kind) && v.x + t <= o.offset)
            .map(|v| v.x + t)
            .max()
            .unwrap_or(t);
        let right_stud = verticals
            .iter()
            .filter(|v| full_height(v.kind) && v.x >= o.offset + o.width)
            .map(|v| v.x)
            .min()
            .unwrap_or(len - t);
        let below = o.sill_height - t;
        let above = o.sill_height + o.height;
        let name = format!("penetration {}", f.index + 1);
        if below < plate_top || above + t > top_plate_bottom {
            return Err(fail(format!(
                "{name} needs room for a block above and below it"
            )));
        }
        for y in [below, above] {
            horizontals.push(Horizontal {
                kind: MemberKind::Blocking,
                size: lumber,
                rect: Rect {
                    x: left_stud,
                    y,
                    w: right_stud - left_stud,
                    h: t,
                },
                ply: 0,
            });
        }
    }

    // Plates. Doors at floor level cut the bottom plate out across the opening.
    let mut cutouts: Vec<(Length, Length)> = framed
        .iter()
        .filter(|f| !f.blocked && f.opening.sill_height == Length::ZERO)
        .map(|f| (f.opening.offset, f.opening.offset + f.opening.width))
        .collect();
    cutouts.sort();
    let mut start = Length::ZERO;
    let mut bottom = Vec::new();
    for (a, b) in cutouts.into_iter().chain([(len, len)]) {
        if a > start {
            bottom.push((start, a));
        }
        start = b;
    }

    let mut members = Vec::new();
    let label = |prefix: &str, n: usize, width: usize| format!("{}-{prefix}{n:0width$}", wall.id);
    for (n, (a, b)) in bottom.into_iter().enumerate() {
        members.push(placed(
            label("BP", n + 1, 1),
            MemberKind::BottomPlate,
            lumber,
            Rect {
                x: a,
                y: Length::ZERO,
                w: b - a,
                h: t,
            },
            false,
        ));
    }
    for (prefix, kind, y) in [
        ("TP", MemberKind::TopPlate, top_plate_bottom),
        ("CP", MemberKind::CapPlate, height - t),
    ] {
        members.push(placed(
            label(prefix, 1, 1),
            kind,
            lumber,
            Rect {
                x: Length::ZERO,
                y,
                w: len,
                h: t,
            },
            false,
        ));
    }

    verticals.sort_by_key(|v| (v.x, v.y));
    let mut counts = [0usize; 5];
    for v in &verticals {
        let (slot, prefix, width) = match v.kind {
            MemberKind::Stud => (0, "S", 2),
            MemberKind::KingStud => (1, "K", 1),
            MemberKind::JackStud => (2, "J", 1),
            MemberKind::Cripple => (3, "C", 2),
            _ => (4, "B", 1),
        };
        counts[slot] += 1;
        members.push(placed(
            label(prefix, counts[slot], width),
            v.kind,
            lumber,
            Rect {
                x: v.x,
                y: v.y,
                w: t,
                h: v.h,
            },
            true,
        ));
    }

    horizontals.sort_by_key(|h| (h.kind == MemberKind::Blocking, h.rect.x, h.rect.y));
    let mut header_count = 0;
    let mut sill_count = 0;
    for h in &horizontals {
        let name = match h.kind {
            MemberKind::Header => {
                // Plies of one header share a number: H1a, H1b.
                if h.ply == 0 {
                    header_count += 1;
                }
                format!("{}-H{header_count}{}", wall.id, (b'a' + h.ply) as char)
            }
            MemberKind::Sill => {
                sill_count += 1;
                label("SL", sill_count, 1)
            }
            _ => {
                counts[4] += 1;
                label("B", counts[4], 1)
            }
        };
        members.push(placed(name, h.kind, h.size, h.rect, false));
    }

    let mut marks: Vec<LayoutMark> = members
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
        .map(|p| LayoutMark {
            label: p.member.label.clone(),
            kind: p.member.kind,
            at: match rules.layout_from {
                LayoutStart::Left => p.rect.x,
                LayoutStart::Right => len - p.rect.right(),
            },
        })
        .collect();
    marks.sort_by_key(|m| m.at);
    marks.dedup_by_key(|m| m.at);

    Ok(FramedWall {
        id: wall.id.clone(),
        length: len,
        height,
        members,
        marks,
        warnings,
    })
}

fn placed(label: String, kind: MemberKind, size: Nominal, rect: Rect, vertical: bool) -> Placed {
    Placed {
        member: Member {
            label,
            kind,
            size,
            length: if vertical { rect.h } else { rect.w },
            cuts: Vec::new(),
        },
        rect,
    }
}

/// Left edges of on-center studs, excluding the two end studs.
fn layout_positions(len: Length, rules: &Rules, t: Length) -> Vec<Length> {
    let mut out = Vec::new();
    let mut k = 1;
    loop {
        let center = rules.on_center * k;
        let x = match rules.layout_from {
            LayoutStart::Left => center - half(t),
            LayoutStart::Right => len - center - half(t),
        };
        if x < t || x + t > len - t {
            break;
        }
        out.push(x);
        k += 1;
    }
    out.sort();
    out
}

fn fixed_zones(base: &[(Length, Length)], framed: &[Framed], t: Length) -> Vec<(Length, Length)> {
    let mut zones = base.to_vec();
    zones.extend(framed.iter().filter(|f| !f.blocked).map(|f| {
        (
            f.opening.offset - t * 2,
            f.opening.offset + f.opening.width + t * 2,
        )
    }));
    zones
}

fn full_height(kind: MemberKind) -> bool {
    matches!(kind, MemberKind::Stud | MemberKind::KingStud)
}

fn end_reserve(end: WallEnd, t: Length) -> Length {
    match end {
        WallEnd::Plain => t,
        WallEnd::Corner => t * 3,
    }
}

/// How many header plies fill the wall's depth (two 2x plies in a 2x4 wall).
fn header_plies(depth: Length, ply: Length) -> u8 {
    (depth.sixteenths() / ply.sixteenths()).clamp(1, 4) as u8
}

/// `n` pieces of length `piece` spread evenly from `start` over `span`.
fn spread(start: Length, span: Length, piece: Length, n: i64) -> Vec<Length> {
    if n == 1 {
        return vec![start + half(span - piece)];
    }
    let step = (span - piece).sixteenths() / (n - 1);
    (0..n)
        .map(|i| start + Length::from_sixteenths(step * i))
        .collect()
}

fn half(l: Length) -> Length {
    Length::from_sixteenths(l.sixteenths() / 2)
}

fn overlaps(a: (Length, Length), b: (Length, Length)) -> bool {
    a.0 < b.1 && b.0 < a.1
}

fn kind_name(kind: OpeningKind) -> &'static str {
    match kind {
        OpeningKind::Door => "door",
        OpeningKind::Window => "window",
        OpeningKind::Penetration => "penetration",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use sticks_core::Project;

    fn l(s: &str) -> Length {
        s.parse().unwrap()
    }

    fn frame(toml: &str) -> Result<FramedWall, FramingError> {
        let p = Project::from_toml(toml).unwrap();
        frame_wall(&p.walls[0], &p.rules)
    }

    fn find<'a>(w: &'a FramedWall, label: &str) -> &'a Placed {
        w.members
            .iter()
            .find(|p| p.member.label == label)
            .unwrap_or_else(|| panic!("no {label}"))
    }

    fn of_kind(w: &FramedWall, kind: MemberKind) -> Vec<&Placed> {
        w.members.iter().filter(|p| p.member.kind == kind).collect()
    }

    const DOOR_WALL: &str = r#"
name = "Partition"

[[wall]]
id = "W1"
length = "12'"
height = "97-1/8\""

[[wall.opening]]
kind = "door"
offset = "36\""
width = "38-1/2\""
height = "82-1/2\""
header = "2x8"
"#;

    /// Worked by hand on paper: a 12' partition, studs 16" on center from the
    /// left, a 38-1/2" x 82-1/2" door rough opening 36" from the left end.
    #[test]
    fn door_wall_matches_hand_layout() {
        let w = frame(DOOR_WALL).unwrap();

        // Studs are 97-1/8 minus three 1-1/2 plates.
        for p in of_kind(&w, MemberKind::Stud)
            .into_iter()
            .chain(of_kind(&w, MemberKind::KingStud))
        {
            assert_eq!(p.member.length, l("92-5/8"), "{}", p.member.label);
        }

        let edges = |kind| -> Vec<String> {
            of_kind(&w, kind)
                .iter()
                .map(|p| format!("{:#}", p.rect.x))
                .collect()
        };
        assert_eq!(
            edges(MemberKind::Stud),
            [
                "0\"",
                "15-1/4\"",
                "31-1/4\"",
                "79-1/4\"",
                "95-1/4\"",
                "111-1/4\"",
                "127-1/4\"",
                "142-1/2\""
            ]
        );
        assert_eq!(edges(MemberKind::KingStud), ["33\"", "76\""]);
        assert_eq!(edges(MemberKind::JackStud), ["34-1/2\"", "74-1/2\""]);
        for jack in of_kind(&w, MemberKind::JackStud) {
            assert_eq!(jack.member.length, l("81"));
        }

        // Two 2x8 plies spanning the opening plus both jacks.
        let headers = of_kind(&w, MemberKind::Header);
        assert_eq!(headers.len(), 2);
        assert_eq!(find(&w, "W1-H1a").member.length, l("41-1/2"));
        assert_eq!(find(&w, "W1-H1b").rect, headers[0].rect);
        assert_eq!(headers[0].rect.y, l("82-1/2"));

        // Cripples on layout above the header: 94-1/8 - 89-3/4.
        let cripples = of_kind(&w, MemberKind::Cripple);
        assert_eq!(cripples.len(), 2);
        assert_eq!(cripples[0].rect.x, l("47-1/4"));
        assert_eq!(cripples[1].rect.x, l("63-1/4"));
        assert_eq!(cripples[0].member.length, l("4-3/8"));

        // The bottom plate is cut out across the door.
        let bottom: Vec<_> = of_kind(&w, MemberKind::BottomPlate)
            .iter()
            .map(|p| (p.rect.x, p.member.length))
            .collect();
        assert_eq!(bottom, [(l("0"), l("36")), (l("74-1/2"), l("69-1/2"))]);
        assert_eq!(find(&w, "W1-TP1").member.length, l("12'"));
        assert_eq!(find(&w, "W1-CP1").rect.y, l("95-5/8"));

        assert_eq!(find(&w, "W1-S01").rect.x, Length::ZERO);
        assert_eq!(find(&w, "W1-S08").rect.x, l("142-1/2"));
        assert_eq!(find(&w, "W1-K2").rect.x, l("76"));
        assert!(w.warnings.is_empty());
    }

    #[test]
    fn marks_measure_from_the_layout_end() {
        let w = frame(DOOR_WALL).unwrap();
        let first: Vec<_> = w.marks.iter().take(3).map(|m| m.at).collect();
        assert_eq!(first, [l("0"), l("15-1/4"), l("31-1/4")]);

        let right = DOOR_WALL.replace(
            "name = \"Partition\"",
            "name = \"P\"\n[rules]\nlayout_from = \"right\"",
        );
        let w = frame(&right).unwrap();
        // Measured from the right end to each stud's right-hand edge.
        let studs: Vec<_> = of_kind(&w, MemberKind::Stud)
            .iter()
            .map(|p| l("12'") - p.rect.right())
            .collect();
        assert!(studs.contains(&l("15-1/4")));
        assert_eq!(w.marks[1].at, l("15-1/4"));
    }

    #[test]
    fn windows_get_a_sill_and_cripples_below() {
        let w = frame(
            r#"
name = "x"
[[wall]]
id = "W2"
length = "8'"
height = "97-1/8\""
[[wall.opening]]
kind = "window"
offset = "24\""
width = "36\""
height = "36\""
sill_height = "44\""
header = "2x6"
"#,
        )
        .unwrap();
        let sill = find(&w, "W2-SL1");
        assert_eq!(sill.rect.y, l("42-1/2"));
        assert_eq!(sill.member.length, l("36"));
        let cripples = of_kind(&w, MemberKind::Cripple);
        let below: Vec<_> = cripples.iter().filter(|c| c.rect.y == l("1-1/2")).collect();
        let above: Vec<_> = cripples.iter().filter(|c| c.rect.y > l("44")).collect();
        // Layout studs at 31-1/4 and 47-1/4 fall inside the opening.
        assert_eq!(below.len(), 2);
        assert_eq!(below[0].member.length, l("41"));
        assert_eq!(above.len(), 2);
        // Header top 80 + 5-1/2 = 85-1/2; top plate at 94-1/8.
        assert_eq!(above[0].member.length, l("8-5/8"));
        assert_eq!(of_kind(&w, MemberKind::BottomPlate).len(), 1);
    }

    #[test]
    fn corners_and_tees_add_backing() {
        let w = frame(
            r#"
name = "x"
[[wall]]
id = "F"
length = "10'"
height = "8'"
left_end = "corner"
right_end = "corner"
tees = ["4'"]
"#,
        )
        .unwrap();
        let studs: Vec<_> = of_kind(&w, MemberKind::Stud)
            .iter()
            .map(|p| p.rect.x)
            .collect();
        // End and backer studs at each end, a channel either side of 48".
        for x in ["0", "3", "44-3/4", "49-3/4", "115-1/2", "118-1/2"] {
            assert!(studs.contains(&l(x)), "missing stud at {x}");
        }
        // The layout stud at 47-1/4 collides with the channel and is dropped.
        assert!(!studs.contains(&l("47-1/4")));
        assert_eq!(of_kind(&w, MemberKind::Blocking).len(), 6);
    }

    #[test]
    fn small_penetrations_between_studs_get_blocking() {
        let w = frame(
            r#"
name = "x"
[[wall]]
id = "W"
length = "8'"
height = "8'"
[[wall.opening]]
kind = "penetration"
offset = "20\""
width = "4\""
height = "4\""
sill_height = "12\""
header = "2x4"
"#,
        )
        .unwrap();
        assert!(of_kind(&w, MemberKind::Header).is_empty());
        let blocks = of_kind(&w, MemberKind::Blocking);
        assert_eq!(blocks.len(), 2);
        // Between the studs at 15-1/4 and 31-1/4.
        assert_eq!(blocks[0].rect.x, l("16-3/4"));
        assert_eq!(blocks[0].member.length, l("14-1/2"));
    }

    #[test]
    fn penetrations_that_hit_a_stud_get_a_header() {
        let w = frame(
            r#"
name = "x"
[[wall]]
id = "W"
length = "8'"
height = "8'"
[[wall.opening]]
kind = "penetration"
offset = "14\""
width = "4\""
height = "4\""
sill_height = "12\""
header = "2x4"
"#,
        )
        .unwrap();
        assert_eq!(of_kind(&w, MemberKind::Header).len(), 2);
        assert_eq!(of_kind(&w, MemberKind::KingStud).len(), 2);
    }

    #[test]
    fn explains_impossible_walls() {
        let too_tall = DOOR_WALL.replace("82-1/2", "90");
        let err = frame(&too_tall).unwrap_err().to_string();
        assert!(err.contains("too tall"), "{err}");

        let near_end = DOOR_WALL.replace("offset = \"36\\\"\"", "offset = \"2\\\"\"");
        let err = frame(&near_end).unwrap_err().to_string();
        assert!(err.contains("too close to the left end"), "{err}");

        let twice = format!(
            "{DOOR_WALL}\n[[wall.opening]]\nkind = \"window\"\noffset = \"76\\\"\"\nwidth = \"24\\\"\"\nheight = \"24\\\"\"\nsill_height = \"40\\\"\"\nheader = \"2x6\"\n"
        );
        let err = frame(&twice).unwrap_err().to_string();
        assert!(err.contains("overlaps"), "{err}");
    }

    fn arb_wall() -> impl Strategy<Value = (Wall, Rules)> {
        (
            (48i64..360, 84i64..144, prop::bool::ANY, prop::bool::ANY),
            prop::sample::select(vec![12i64, 16, 19, 24]),
            prop::bool::ANY,
            prop::collection::vec((0i64..360 * 16, 4i64..60 * 16, 0i64..60 * 16), 0..3),
        )
            .prop_map(|((len, h, lc, rc), oc, right, raw)| {
                let rules = Rules {
                    on_center: Length::inches(oc),
                    layout_from: if right {
                        LayoutStart::Right
                    } else {
                        LayoutStart::Left
                    },
                    ..Rules::default()
                };
                let openings = raw
                    .into_iter()
                    .map(|(off, w, sill)| Opening {
                        kind: if sill == 0 {
                            OpeningKind::Door
                        } else {
                            OpeningKind::Window
                        },
                        offset: Length::from_sixteenths(off),
                        width: Length::from_sixteenths(w),
                        height: Length::inches(24),
                        sill_height: if sill < 64 {
                            Length::ZERO
                        } else {
                            Length::from_sixteenths(sill)
                        },
                        header: Nominal::new(2, 6),
                    })
                    .collect();
                let end = |c| if c { WallEnd::Corner } else { WallEnd::Plain };
                let wall = Wall {
                    id: "P".into(),
                    length: Length::inches(len),
                    height: Length::inches(h),
                    left_end: end(lc),
                    right_end: end(rc),
                    tees: Vec::new(),
                    openings,
                };
                (wall, rules)
            })
    }

    proptest! {
        /// Whatever frames must fit inside the wall, with no two full-height
        /// members in the same place and no stud bay wider than the spacing.
        #[test]
        fn framed_walls_are_sound((wall, rules) in arb_wall()) {
            let Ok(w) = frame_wall(&wall, &rules) else { return Ok(()) };
            for p in &w.members {
                prop_assert!(p.rect.x >= Length::ZERO && p.rect.right() <= w.length, "{}", p.member.label);
                prop_assert!(p.rect.y >= Length::ZERO && p.rect.y + p.rect.h <= w.height, "{}", p.member.label);
                prop_assert!(p.member.length > Length::ZERO, "{}", p.member.label);
            }
            let mut tall: Vec<_> = w.members.iter()
                .filter(|p| full_height(p.member.kind))
                .map(|p| p.rect)
                .collect();
            tall.sort_by_key(|r| r.x);
            for pair in tall.windows(2) {
                prop_assert!(pair[0].right() <= pair[1].x, "overlap at {}", pair[1].x);
            }
            let labels: std::collections::HashSet<_> = w.members.iter().map(|p| &p.member.label).collect();
            prop_assert_eq!(labels.len(), w.members.len());
            // Above every point of the wall sits a stud, cripple, or opening within one spacing.
            for pair in tall.windows(2) {
                let gap = pair[1].x - pair[0].right();
                let spans_opening = wall.openings.iter().any(|o| o.offset < pair[1].x && o.offset + o.width > pair[0].right());
                if !spans_opening {
                    prop_assert!(gap < rules.on_center, "bay of {} at {}", gap, pair[0].x);
                }
            }
        }
    }
}
