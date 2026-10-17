//! Stud walls: plates, layout studs, corners, tees and openings.
//!
//! Coordinates are the wall's elevation seen from outside: `x` runs from the
//! wall's left end, `y` from the bottom of the bottom plate.
//!
//! A wall may be level or a rake wall, whose top runs straight from one
//! height at its left end to another at its right, as under a lean-to roof.
//! In a rake wall the top and cap plates follow the slope and every stud
//! gets an angled top cut.

use serde::Serialize;
use sticks_core::{
    longest_board, tenths_of_degree, Cut, LayoutStart, Length, MemberKind, Nominal, Opening,
    OpeningKind, Rules, Stock, Wall, WallEnd,
};

use crate::layout::{
    half, on_center, overlaps, pieces, placed, split_run, spread, LayoutMark, Placed, Rect, Splice,
};

/// Spacer blocks in a three-stud corner: how many, and how long each is.
const CORNER_BLOCKS: i64 = 3;
const CORNER_BLOCK_LENGTH: Length = Length::inches(12);
/// Cripples shorter than this are hard to nail without splitting.
const SHORT_CRIPPLE: Length = Length::inches(3);
/// Joints in the top plate and the cap plate stay at least this far apart.
pub const PLATE_JOINT_OFFSET: Length = Length::inches(24);

/// The outline a wall is framed to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WallShape {
    pub length: Length,
    /// Subfloor to top of cap plate at the wall's left end.
    pub left_height: Length,
    /// Subfloor to top of cap plate at the wall's right end.
    pub right_height: Length,
    pub left_end: WallEnd,
    pub right_end: WallEnd,
}

/// A rough opening as drawn in the elevation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OpeningOutline {
    pub kind: OpeningKind,
    /// `Door 1`, `Penetration 2`: numbered in the order the project lists them.
    pub name: String,
    pub rect: Rect,
}

/// Everything generated for one wall.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FramedWall {
    pub id: String,
    pub length: Length,
    /// The taller of the two end heights.
    pub height: Length,
    pub left_height: Length,
    pub right_height: Length,
    pub members: Vec<Placed>,
    /// Vertical members, ordered from the layout end.
    pub marks: Vec<LayoutMark>,
    /// Plate joints, each centered on a stud.
    pub splices: Vec<Splice>,
    pub openings: Vec<OpeningOutline>,
    /// Things that frame but deserve a second look.
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, thiserror::Error)]
#[error("{part}: {reason}")]
pub struct FramingError {
    /// What failed to frame, such as `wall front`, `floor` or `roof`.
    pub part: String,
    pub reason: String,
}

/// A vertical member before it gets a label.
struct Vertical {
    kind: MemberKind,
    x: Length,
    y: Length,
    /// Height to the long point of the top.
    h: Length,
    /// Whether the top meets the top plate, and so takes its slope.
    to_plate: bool,
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

/// The line along the top of the wall, and the plates under it.
struct Top {
    len: Length,
    left: Length,
    right: Length,
    /// Plumb thickness of one plate: more than its actual thickness on a slope.
    plate: Length,
}

impl Top {
    fn new(shape: &WallShape, t: Length) -> Top {
        let rise = (shape.right_height - shape.left_height).as_inches_f64();
        let run = shape.length.as_inches_f64();
        let plate = if rise == 0.0 {
            t
        } else {
            Length::from_inches_f64(t.as_inches_f64() * (1.0 + (rise / run).powi(2)).sqrt())
        };
        Top {
            len: shape.length,
            left: shape.left_height,
            right: shape.right_height,
            plate,
        }
    }

    fn sloped(&self) -> bool {
        self.left != self.right
    }

    /// Top of the cap plate at `x`.
    fn at(&self, x: Length) -> Length {
        self.left + (self.right - self.left).mul_div(x.sixteenths(), self.len.sixteenths().max(1))
    }

    /// Bottom of the top plate at `x`: where studs end.
    fn studs_end(&self, x: Length) -> Length {
        self.at(x) - self.plate * 2
    }

    /// Short and long point of a stud's top across `[x, x + t]`.
    fn column(&self, x: Length, t: Length) -> (Length, Length) {
        let (a, b) = (self.studs_end(x), self.studs_end(x + t));
        (a.min(b), a.max(b))
    }

    /// Lowest point of the top plate's underside over `[a, b]`.
    fn lowest(&self, a: Length, b: Length) -> Length {
        self.studs_end(a).min(self.studs_end(b))
    }

    /// The slope's angle from level, in tenths of a degree.
    fn angle(&self) -> i32 {
        tenths_of_degree((self.right - self.left).abs(), self.len)
    }

    /// Length along the slope for a level `span`.
    fn along(&self, span: Length) -> Length {
        if !self.sloped() {
            return span;
        }
        let rise = (self.right - self.left).as_inches_f64() / self.len.as_inches_f64();
        Length::from_inches_f64(span.as_inches_f64() * (1.0 + rise * rise).sqrt())
    }
}

impl WallShape {
    /// The shape a free-standing wall gives in its own fields.
    pub fn of(wall: &Wall) -> Result<WallShape, String> {
        let length = wall
            .length
            .ok_or_else(|| "give the wall a length".to_string())?;
        let height = wall
            .height
            .ok_or_else(|| "give the wall a height".to_string())?;
        Ok(WallShape {
            length,
            left_height: height,
            right_height: height,
            left_end: wall.left_end.unwrap_or_default(),
            right_end: wall.right_end.unwrap_or_default(),
        })
    }
}

/// Frames a wall that gives its own length and height.
pub fn frame_wall(wall: &Wall, rules: &Rules, stock: &[Stock]) -> Result<FramedWall, FramingError> {
    let shape = WallShape::of(wall).map_err(|reason| FramingError {
        part: format!("wall {}", wall.id),
        reason,
    })?;
    frame_wall_shaped(wall, &shape, rules, stock)
}

/// Frames a wall to `shape`, taking its openings and tees from `wall`. Plates
/// longer than the longest stock board are split with joints on studs.
pub fn frame_wall_shaped(
    wall: &Wall,
    shape: &WallShape,
    rules: &Rules,
    stock: &[Stock],
) -> Result<FramedWall, FramingError> {
    let fail = |reason: String| FramingError {
        part: format!("wall {}", wall.id),
        reason,
    };
    let lumber = rules.wall_lumber;
    let t = lumber.actual_thickness();
    let depth = lumber.actual_width();
    let len = shape.length;
    let height = shape.left_height.max(shape.right_height);
    let top = Top::new(shape, t);
    let plate_top = t;

    if rules.on_center < t * 2 {
        return Err(fail(format!(
            "studs {:#} on center leaves no room between studs",
            rules.on_center
        )));
    }
    if len <= Length::ZERO {
        return Err(fail("needs a length".to_string()));
    }
    let low = shape.left_height.min(shape.right_height);
    if low - top.plate * 2 - plate_top <= Length::ZERO {
        return Err(fail(format!(
            "{low} is too short for three {} plates",
            lumber
        )));
    }
    let left_reserve = end_reserve(shape.left_end, t);
    let right_reserve = end_reserve(shape.right_end, t);
    if len < left_reserve + right_reserve {
        return Err(fail(format!("{len} is too short for its end studs")));
    }

    // A stud from the bottom plate up to the top plate.
    let full = |kind: MemberKind, x: Length| Vertical {
        kind,
        x,
        y: plate_top,
        h: top.column(x, t).1 - plate_top,
        to_plate: true,
    };

    // Zones that layout studs must stay out of, as [start, end) on x.
    let mut zones = vec![(Length::ZERO, left_reserve), (len - right_reserve, len)];
    let mut verticals = vec![
        full(MemberKind::Stud, Length::ZERO),
        full(MemberKind::Stud, len - t),
    ];
    let mut horizontals = Vec::new();
    for (end, block_x, backer_x) in [
        (shape.left_end, t, t * 2),
        (shape.right_end, len - t * 2, len - t * 3),
    ] {
        if end == WallEnd::Corner {
            verticals.push(full(MemberKind::Stud, backer_x));
            let span = top.column(block_x, t).0 - plate_top;
            for y in spread(plate_top, span, CORNER_BLOCK_LENGTH, CORNER_BLOCKS) {
                verticals.push(Vertical {
                    kind: MemberKind::Blocking,
                    x: block_x,
                    y,
                    h: CORNER_BLOCK_LENGTH,
                    to_plate: false,
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
            verticals.push(full(MemberKind::Stud, x));
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
    let layout = on_center(len, rules.on_center, t, rules.layout_from);
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
        let ceiling = top.lowest(o.offset - t, o.offset + o.width + t);
        if header_top > ceiling {
            return Err(fail(format!(
                "{name} is too tall: its {} header would end at {:#}, but the top plates start at {:#}",
                o.header, header_top, ceiling
            )));
        }
    }

    let fixed = fixed_zones(&zones, &framed, t);
    let mut warnings = Vec::new();
    for &x in &layout {
        if !fixed.iter().any(|&z| overlaps((x, x + t), z)) {
            verticals.push(full(MemberKind::Stud, x));
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
            verticals.push(full(MemberKind::KingStud, x));
        }
        for x in [left - t, right] {
            verticals.push(Vertical {
                kind: MemberKind::JackStud,
                x,
                y: plate_top,
                h: head_y - plate_top,
                to_plate: false,
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

        let top_columns: Vec<Length> = layout
            .iter()
            .copied()
            .filter(|&x| x >= header.x && x + t <= header.right())
            .filter(|&x| top.column(x, t).0 > header_top)
            .collect();
        if top_columns
            .iter()
            .any(|&x| top.column(x, t).0 - header_top < SHORT_CRIPPLE)
        {
            let shortest = top_columns
                .iter()
                .map(|&x| top.column(x, t).0 - header_top)
                .min()
                .unwrap_or_default();
            warnings.push(format!(
                "{name}: cripples above the header are only {:#} long; a deeper header would fill the gap",
                shortest
            ));
        }
        for x in top_columns {
            verticals.push(Vertical {
                kind: MemberKind::Cripple,
                x,
                y: header_top,
                h: top.column(x, t).1 - header_top,
                to_plate: true,
            });
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
                    to_plate: false,
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
        if below < plate_top || above + t > top.lowest(left_stud, right_stud) {
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

    // Plates longer than any board are spliced over a stud, the top and cap
    // plate joints kept apart.
    let longest = longest_board(stock, lumber, rules.end_trim);
    let centers: Vec<Length> = verticals
        .iter()
        .filter(|v| full_height(v.kind))
        .map(|v| v.x + half(t))
        .collect();
    let too_long = |kind: MemberKind| {
        fail(format!(
            "the {} is longer than the longest {lumber} stock ({longest}) and no stud lands where it could be spliced; add longer stock",
            kind.name()
        ))
    };
    let mut splices = Vec::new();
    let mut bottom_pieces = Vec::new();
    for (a, b) in bottom {
        let on_plate: Vec<Length> = centers
            .iter()
            .copied()
            .filter(|&c| c - half(t) >= a && c + half(t) <= b)
            .collect();
        let joints = split_run(a, b, |s| s <= longest, &on_plate, &[], Length::ZERO)
            .ok_or_else(|| too_long(MemberKind::BottomPlate))?;
        splices.extend(joints.iter().map(|&at| Splice {
            kind: MemberKind::BottomPlate,
            at,
        }));
        bottom_pieces.extend(pieces(a, b, &joints));
    }
    let fits_sloped = |s: Length| top.along(s) <= longest;
    let top_joints = split_run(Length::ZERO, len, fits_sloped, &centers, &[], Length::ZERO)
        .ok_or_else(|| too_long(MemberKind::TopPlate))?;
    let cap_joints = split_run(
        Length::ZERO,
        len,
        fits_sloped,
        &centers,
        &top_joints,
        PLATE_JOINT_OFFSET,
    )
    .ok_or_else(|| {
        fail(format!(
            "the cap plate needs a splice at least {:#} from the top plate's, over a stud, and none fits within the longest {lumber} stock ({longest}); add longer stock",
            PLATE_JOINT_OFFSET
        ))
    })?;
    for (kind, joints) in [
        (MemberKind::TopPlate, &top_joints),
        (MemberKind::CapPlate, &cap_joints),
    ] {
        splices.extend(joints.iter().map(|&at| Splice { kind, at }));
    }
    splices.sort_by_key(|s| s.at);

    let mut members = Vec::new();
    let label = |prefix: &str, n: usize, width: usize| format!("{}-{prefix}{n:0width$}", wall.id);
    for (n, (a, b)) in bottom_pieces.into_iter().enumerate() {
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
    for (prefix, kind, below_top, joints) in [
        ("TP", MemberKind::TopPlate, top.plate * 2, &top_joints),
        ("CP", MemberKind::CapPlate, top.plate, &cap_joints),
    ] {
        for (n, (a, b)) in pieces(Length::ZERO, len, joints).into_iter().enumerate() {
            let (ya, yb) = (top.at(a) - below_top, top.at(b) - below_top);
            let mut p = placed(
                label(prefix, n + 1, 1),
                kind,
                lumber,
                Rect {
                    x: a,
                    y: ya.min(yb),
                    w: b - a,
                    h: (yb - ya).abs() + top.plate,
                },
                false,
            );
            if top.sloped() {
                p.member.length = top.along(b - a);
                let cut = Cut::Angled {
                    tenths_of_degree: top.angle(),
                };
                p.member.cuts = vec![cut, cut];
                p.outline = vec![(a, ya), (b, yb), (b, yb + top.plate), (a, ya + top.plate)];
            }
            members.push(p);
        }
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
        let mut p = placed(
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
        );
        if v.to_plate && top.sloped() {
            p.member.cuts = vec![Cut::Angled {
                tenths_of_degree: top.angle(),
            }];
            p.outline = vec![
                (v.x, v.y),
                (v.x + t, v.y),
                (v.x + t, top.studs_end(v.x + t)),
                (v.x, top.studs_end(v.x)),
            ];
        }
        members.push(p);
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

    let openings = wall
        .openings
        .iter()
        .enumerate()
        .map(|(i, o)| OpeningOutline {
            kind: o.kind,
            name: format!("{} {}", capitalized(kind_name(o.kind)), i + 1),
            rect: Rect {
                x: o.offset,
                y: o.sill_height,
                w: o.width,
                h: o.height,
            },
        })
        .collect();

    Ok(FramedWall {
        id: wall.id.clone(),
        length: len,
        height,
        left_height: shape.left_height,
        right_height: shape.right_height,
        members,
        marks,
        splices,
        openings,
        warnings,
    })
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

fn kind_name(kind: OpeningKind) -> &'static str {
    match kind {
        OpeningKind::Door => "door",
        OpeningKind::Window => "window",
        OpeningKind::Penetration => "penetration",
    }
}

fn capitalized(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
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
        frame_wall(&p.walls[0], &p.rules, &p.stock)
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

    #[test]
    fn long_plates_split_over_studs() {
        let w = frame(
            r#"
name = "x"
[[stock]]
size = "2x4"
lengths = ["8'", "16'"]
[[wall]]
id = "F"
length = "18'"
height = "8'"
"#,
        )
        .unwrap();
        // The top plate runs a full 16' to the center of the stud at 191-1/4.
        let tp: Vec<_> = of_kind(&w, MemberKind::TopPlate)
            .iter()
            .map(|p| (p.member.label.clone(), p.rect.x, p.member.length))
            .collect();
        assert_eq!(
            tp,
            [
                ("F-TP1".to_string(), l("0"), l("16'")),
                ("F-TP2".to_string(), l("16'"), l("24"))
            ]
        );
        // The cap plate's joint is two feet or more from the top plate's.
        let cp: Vec<_> = of_kind(&w, MemberKind::CapPlate)
            .iter()
            .map(|p| p.member.length)
            .collect();
        assert_eq!(cp, [l("160"), l("56")]);
        assert_eq!(of_kind(&w, MemberKind::BottomPlate).len(), 2);
        for s in &w.splices {
            let center = s.at;
            assert!(
                of_kind(&w, MemberKind::Stud)
                    .iter()
                    .any(|p| p.rect.x + l("3/4") == center),
                "splice at {center} isn't on a stud"
            );
        }
        assert_eq!(w.splices.len(), 3);

        let none = frame(
            "name = \"x\"\n[[stock]]\nsize = \"2x4\"\nlengths = [\"8'\"]\n[[wall]]\nid = \"F\"\nlength = \"18'\"\nheight = \"8'\"\n",
        )
        .unwrap();
        assert_eq!(of_kind(&none, MemberKind::TopPlate).len(), 3);
    }

    #[test]
    fn rake_walls_follow_the_slope() {
        let p = Project::from_toml("name = \"x\"\n[[wall]]\nid = \"R\"\n").unwrap();
        // 3:12 over 12' drops 36".
        let shape = WallShape {
            length: l("12'"),
            left_height: l("10'"),
            right_height: l("7'"),
            left_end: WallEnd::Plain,
            right_end: WallEnd::Plain,
        };
        let w = frame_wall_shaped(&p.walls[0], &shape, &p.rules, &[]).unwrap();
        assert_eq!(w.height, l("10'"));
        // Plates are plumb-cut and run the slope: 144 * sqrt(1 + 1/16).
        let cap = find(&w, "R-CP1");
        assert_eq!(cap.member.length, l("148-7/16"));
        assert_eq!(cap.member.cuts.len(), 2);
        assert_eq!(cap.outline.len(), 4);
        // The left end stud runs to its long point under two 1-9/16" plates.
        let first = find(&w, "R-S01");
        assert_eq!(first.member.length, l("120") - l("3-1/8") - l("1-1/2"));
        assert_eq!(
            first.member.cuts,
            [Cut::Angled {
                tenths_of_degree: 140
            }]
        );
        let studs = of_kind(&w, MemberKind::Stud);
        assert!(studs
            .windows(2)
            .all(|p| p[0].member.length > p[1].member.length));
        assert_eq!(find(&w, "R-BP1").member.length, l("12'"));
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
                    length: Some(Length::inches(len)),
                    height: Some(Length::inches(h)),
                    left_end: Some(end(lc)),
                    right_end: Some(end(rc)),
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
            let Ok(w) = frame_wall(&wall, &rules, &[]) else { return Ok(()) };
            check_sound(&wall, &rules, &w)?;
        }

        /// The same holds for rake walls, and every plate fits stock once split.
        #[test]
        fn rake_walls_are_sound((wall, rules) in arb_wall(), drop in 0i64..48 * 16, low_right in any::<bool>()) {
            let mut shape = WallShape::of(&wall).unwrap();
            let low = shape.left_height - Length::from_sixteenths(drop);
            if low_right { shape.right_height = low } else { shape.left_height = low }
            let Ok(w) = frame_wall_shaped(&wall, &shape, &rules, &[]) else { return Ok(()) };
            check_sound(&wall, &rules, &w)?;
            for p in &w.members {
                prop_assert!(p.member.length <= Length::feet(16), "{} is {}", p.member.label, p.member.length);
            }
        }
    }

    fn check_sound(wall: &Wall, rules: &Rules, w: &FramedWall) -> Result<(), TestCaseError> {
        for p in &w.members {
            prop_assert!(
                p.rect.x >= Length::ZERO && p.rect.right() <= w.length,
                "{}",
                p.member.label
            );
            prop_assert!(
                p.rect.y >= Length::ZERO && p.rect.y + p.rect.h <= w.height,
                "{}",
                p.member.label
            );
            prop_assert!(p.member.length > Length::ZERO, "{}", p.member.label);
        }
        let mut tall: Vec<_> = w
            .members
            .iter()
            .filter(|p| full_height(p.member.kind))
            .map(|p| p.rect)
            .collect();
        tall.sort_by_key(|r| r.x);
        for pair in tall.windows(2) {
            prop_assert!(pair[0].right() <= pair[1].x, "overlap at {}", pair[1].x);
        }
        let labels: std::collections::HashSet<_> =
            w.members.iter().map(|p| &p.member.label).collect();
        prop_assert_eq!(labels.len(), w.members.len());
        // Above every point of the wall sits a stud, cripple, or opening within
        // one spacing. A layout stud displaced by a king stud can leave one bay
        // up to a stud's thickness wider.
        let t = rules.wall_lumber.actual_thickness();
        for pair in tall.windows(2) {
            let gap = pair[1].x - pair[0].right();
            let spans_opening = wall
                .openings
                .iter()
                .any(|o| o.offset < pair[1].x && o.offset + o.width > pair[0].right());
            if !spans_opening {
                prop_assert!(gap < rules.on_center + t, "bay of {} at {}", gap, pair[0].x);
            }
        }
        Ok(())
    }
}
