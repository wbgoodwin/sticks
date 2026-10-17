//! Printable drawings: wall elevations, floor and roof framing plans, the
//! subfloor layout and a rafter detail. Each is one US letter page in
//! landscape, laid out in points (1/72").

use std::fmt::Write;

use sticks_core::{LayoutStart, Length, MemberKind};
use sticks_rules::{FramedFloor, FramedRoof, FramedWall, LayoutMark, Placed, Splice};

/// Printed on every drawing: the tool frames what it's told and checks nothing.
pub const NOTICE: &str =
    "Member sizes are chosen by the user and unverified. Sticks does not size members or check spans.";

const PAGE_W: f64 = 792.0;
const PAGE_H: f64 = 612.0;
const MARGIN: f64 = 36.0;
/// Room under a drawing for rows of layout marks.
const MARK_ROW: f64 = 40.0;
const MARK_ROWS: usize = 3;
/// Rotated mark text needs about this much room across.
const MARK_GAP: f64 = 8.0;

/// Maps a drawing's inches onto the page, `y` up.
struct Frame {
    scale: f64,
    x0: f64,
    y0: f64,
}

impl Frame {
    /// Fits `w` by `h` inches, starting at (`min_x`, `min_y`), into the box
    /// `[left, right] x [top, bottom]` on the page, centered across, at the
    /// largest architect's scale that fits.
    fn fit(min_x: f64, min_y: f64, w: f64, h: f64, area: (f64, f64, f64, f64)) -> Frame {
        let (left, top, right, bottom) = area;
        let fit = ((right - left) / w).min((bottom - top) / h);
        let scale = SCALES
            .iter()
            .rev()
            .map(|(per_foot, _)| per_foot * 72.0 / 12.0)
            .find(|&s| s <= fit)
            .unwrap_or(fit);
        let x0 = left + ((right - left) - w * scale) / 2.0 - min_x * scale;
        let y0 = bottom + min_y * scale;
        Frame { scale, x0, y0 }
    }

    fn x(&self, l: Length) -> f64 {
        self.x0 + l.as_inches_f64() * self.scale
    }

    fn y(&self, l: Length) -> f64 {
        self.y0 - l.as_inches_f64() * self.scale
    }

    fn len(&self, l: Length) -> f64 {
        l.as_inches_f64() * self.scale
    }

    /// "1/4\" = 1'" style scale note, to the nearest architect's scale.
    fn note(&self) -> String {
        // Inches of paper per foot of building.
        let per_foot = self.scale * 12.0 / 72.0;
        let best = SCALES
            .iter()
            .min_by(|a, b| (a.0 - per_foot).abs().total_cmp(&(b.0 - per_foot).abs()))
            .unwrap();
        if (best.0 - per_foot).abs() / per_foot < 0.01 {
            format!("Scale {} when printed at 100%", best.1)
        } else {
            format!(
                "Scale about {}; measure from the dimensions, not the drawing",
                best.1
            )
        }
    }
}

/// Architect's scales: inches of paper per foot, smallest first.
const SCALES: [(f64, &str); 11] = [
    (1.0 / 16.0, "1/16\" = 1'"),
    (3.0 / 32.0, "3/32\" = 1'"),
    (1.0 / 8.0, "1/8\" = 1'"),
    (3.0 / 16.0, "3/16\" = 1'"),
    (1.0 / 4.0, "1/4\" = 1'"),
    (3.0 / 8.0, "3/8\" = 1'"),
    (1.0 / 2.0, "1/2\" = 1'"),
    (3.0 / 4.0, "3/4\" = 1'"),
    (1.0, "1\" = 1'"),
    (1.5, "1-1/2\" = 1'"),
    (3.0, "3\" = 1'"),
];

/// An SVG document being written.
struct Doc {
    out: String,
}

impl Doc {
    fn new(title: &str) -> Doc {
        let mut out = String::new();
        let _ = write!(
            out,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="11in" height="8.5in" viewBox="0 0 {PAGE_W} {PAGE_H}" font-family="Helvetica, Arial, sans-serif">
<title>{}</title>
<style>
.m {{ stroke: #222; stroke-width: 0.5; }}
.plate {{ fill: #e9d8b4; }} .stud {{ fill: #f3e6c8; }} .king {{ fill: #e2c992; }} .jack {{ fill: #d8bb7e; }}
.cripple {{ fill: #f6edd9; }} .header {{ fill: #c9a86a; }} .sill {{ fill: #e2c992; }} .block {{ fill: #d7d0c0; }}
.joist {{ fill: #f3e6c8; }} .rim {{ fill: #e9d8b4; }} .beam {{ fill: none; stroke-dasharray: 4 2; }}
.rafter {{ fill: #f3e6c8; }} .fascia {{ fill: #e2c992; }}
.opening {{ fill: none; stroke: #2a6fb0; stroke-width: 0.75; stroke-dasharray: 3 2; }}
.splice {{ stroke: #c0392b; stroke-width: 1.5; }}
.splice-text {{ fill: #c0392b; font-size: 6px; }}
.dim {{ stroke: #444; stroke-width: 0.5; fill: none; }}
.dim-text {{ font-size: 8px; fill: #222; }}
.mark {{ stroke: #888; stroke-width: 0.4; }}
.mark-text {{ font-size: 6.5px; fill: #222; }}
.label {{ font-size: 4.5px; fill: #555; }}
.title {{ font-size: 14px; font-weight: bold; fill: #111; }}
.sub {{ font-size: 8px; fill: #444; }}
.notice {{ font-size: 7px; fill: #666; }}
.pier {{ fill: #fff; stroke: #222; stroke-width: 0.6; }}
.sheet {{ fill: #f6f1e4; stroke: #222; stroke-width: 0.6; }}
</style>
<rect width="{PAGE_W}" height="{PAGE_H}" fill="#fff"/>
"##,
            esc(title)
        );
        Doc { out }
    }

    fn header(&mut self, title: &str, sub: &[String]) {
        self.text(MARGIN, MARGIN + 6.0, "title", "start", title);
        for (i, line) in sub.iter().enumerate() {
            self.text(
                MARGIN,
                MARGIN + 20.0 + i as f64 * 11.0,
                "sub",
                "start",
                line,
            );
        }
        self.text(MARGIN, PAGE_H - MARGIN + 14.0, "notice", "start", NOTICE);
    }

    fn text(&mut self, x: f64, y: f64, class: &str, anchor: &str, text: &str) {
        let _ = writeln!(
            self.out,
            r#"<text x="{x:.2}" y="{y:.2}" class="{class}" text-anchor="{anchor}">{}</text>"#,
            esc(text)
        );
    }

    fn vtext(&mut self, x: f64, y: f64, class: &str, anchor: &str, text: &str) {
        let _ = writeln!(
            self.out,
            r#"<text x="{x:.2}" y="{y:.2}" class="{class}" text-anchor="{anchor}" transform="rotate(-90 {x:.2} {y:.2})">{}</text>"#,
            esc(text)
        );
    }

    fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, class: &str) {
        let _ = writeln!(
            self.out,
            r#"<line x1="{x1:.2}" y1="{y1:.2}" x2="{x2:.2}" y2="{y2:.2}" class="{class}"/>"#
        );
    }

    fn polygon(&mut self, points: &[(f64, f64)], class: &str) {
        let pts: Vec<String> = points
            .iter()
            .map(|(x, y)| format!("{x:.2},{y:.2}"))
            .collect();
        let _ = writeln!(
            self.out,
            r#"<polygon points="{}" class="{class}"/>"#,
            pts.join(" ")
        );
    }

    fn circle(&mut self, x: f64, y: f64, r: f64, class: &str) {
        let _ = writeln!(
            self.out,
            r#"<circle cx="{x:.2}" cy="{y:.2}" r="{r:.2}" class="{class}"/>"#
        );
    }

    /// A dimension line with end ticks and its text above (or left of) it.
    fn dimension(&mut self, a: (f64, f64), b: (f64, f64), text: &str) {
        self.line(a.0, a.1, b.0, b.1, "dim");
        let vertical = (a.0 - b.0).abs() < 0.01;
        for (x, y) in [a, b] {
            if vertical {
                self.line(x - 4.0, y, x + 4.0, y, "dim");
            } else {
                self.line(x, y - 4.0, x, y + 4.0, "dim");
            }
        }
        let (mx, my) = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        if vertical {
            self.vtext(mx - 4.0, my, "dim-text", "middle", text);
        } else {
            self.text(mx, my - 4.0, "dim-text", "middle", text);
        }
    }

    fn finish(mut self) -> String {
        self.out.push_str("</svg>\n");
        self.out
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn class(kind: MemberKind) -> &'static str {
    match kind {
        MemberKind::BottomPlate | MemberKind::TopPlate | MemberKind::CapPlate => "plate",
        MemberKind::Stud => "stud",
        MemberKind::KingStud => "king",
        MemberKind::JackStud => "jack",
        MemberKind::Cripple => "cripple",
        MemberKind::Header => "header",
        MemberKind::Sill => "sill",
        MemberKind::Blocking => "block",
        MemberKind::RimJoist => "rim",
        MemberKind::Joist => "joist",
        MemberKind::Beam | MemberKind::Skid => "beam",
        MemberKind::Rafter | MemberKind::Ledger => "rafter",
        MemberKind::Fascia | MemberKind::Post => "fascia",
    }
}

/// The part of a label after its prefix: `front-S07` is `S07` on its own drawing.
fn short(label: &str) -> &str {
    label.split_once('-').map_or(label, |(_, rest)| rest)
}

fn shape(f: &Frame, p: &Placed) -> Vec<(f64, f64)> {
    if p.outline.is_empty() {
        let r = p.rect;
        vec![
            (f.x(r.x), f.y(r.y)),
            (f.x(r.right()), f.y(r.y)),
            (f.x(r.right()), f.y(r.top())),
            (f.x(r.x), f.y(r.top())),
        ]
    } else {
        p.outline.iter().map(|&(x, y)| (f.x(x), f.y(y))).collect()
    }
}

/// Draws members once per spot (header plies share one), labeled.
fn members(doc: &mut Doc, f: &Frame, members: &[Placed], labels: bool) {
    let mut drawn = Vec::new();
    for p in members {
        if drawn.contains(&p.rect) {
            continue;
        }
        drawn.push(p.rect);
        doc.polygon(&shape(f, p), &format!("m {}", class(p.member.kind)));
    }
    if !labels {
        return;
    }
    let mut named = Vec::new();
    for p in members {
        if named.contains(&p.rect) {
            continue;
        }
        named.push(p.rect);
        let r = p.rect;
        let (cx, cy) = (f.x(r.x + half(r.w)), f.y(r.y + half(r.h)));
        let text = short(&p.member.label);
        let text = match p.member.kind {
            // Plies share a spot: H1a and H1b read as H1.
            MemberKind::Header => text.trim_end_matches(|c: char| c.is_ascii_lowercase()),
            _ => text,
        };
        if f.len(r.h) > f.len(r.w) && f.len(r.w) >= 2.0 && f.len(r.h) > text.len() as f64 * 3.0 {
            doc.vtext(cx + 1.6, cy, "label", "middle", text);
        } else if f.len(r.w) > text.len() as f64 * 3.0 && f.len(r.h) >= 4.0 {
            doc.text(cx, cy + 1.6, "label", "middle", text);
        }
    }
}

fn half(l: Length) -> Length {
    Length::from_sixteenths(l.sixteenths() / 2)
}

/// Layout marks in rows under `base_y`: each mark's measurement, rotated,
/// at the member's near edge. Crowded marks drop to the next row.
fn marks(
    doc: &mut Doc,
    f: &Frame,
    marks: &[LayoutMark],
    from: LayoutStart,
    run: Length,
    base_y: f64,
) {
    let mut ends: Vec<f64> = Vec::new();
    for m in marks {
        let edge = match from {
            LayoutStart::Left => m.at,
            LayoutStart::Right => run - m.at,
        };
        let x = f.x(edge);
        let row = ends
            .iter()
            .position(|&end| (x - end).abs() >= MARK_GAP)
            .unwrap_or(ends.len());
        let row = if row >= MARK_ROWS {
            // Out of rows: reuse the one whose last mark is farthest away.
            (0..ends.len())
                .max_by(|&a, &b| (x - ends[a]).abs().total_cmp(&(x - ends[b]).abs()))
                .unwrap_or(0)
        } else {
            row
        };
        if row == ends.len() {
            ends.push(x);
        } else {
            ends[row] = x;
        }
        // Text reads upward from the bottom of its row; the tick reaches it.
        let ty = base_y + MARK_ROW * (row as f64 + 1.0);
        doc.line(x, base_y + 2.0, x, ty - MARK_ROW + 6.0, "mark");
        doc.vtext(x + 2.2, ty, "mark-text", "start", &format!("{:#}", m.at));
    }
}

fn origin_note(from: LayoutStart, what: &str) -> String {
    match from {
        LayoutStart::Left => format!("{what} measured from the left end to each near edge"),
        LayoutStart::Right => format!("{what} measured from the right end to each near edge"),
    }
}

/// A red line across each piece that starts at a splice.
fn splices(doc: &mut Doc, f: &Frame, members: &[Placed], list: &[Splice]) {
    for s in list {
        for p in members
            .iter()
            .filter(|p| p.member.kind == s.kind && p.rect.x == s.at)
        {
            let ys: Vec<Length> = if p.outline.is_empty() {
                vec![p.rect.y, p.rect.top()]
            } else {
                p.outline
                    .iter()
                    .filter(|(x, _)| *x == s.at)
                    .map(|&(_, y)| y)
                    .collect()
            };
            let (Some(&lo), Some(&hi)) = (ys.iter().min(), ys.iter().max()) else {
                continue;
            };
            let x = f.x(s.at);
            doc.line(x, f.y(lo) + 3.0, x, f.y(hi) - 3.0, "splice");
        }
    }
}

/// One wall's elevation seen from outside, with stud positions dimensioned
/// from the layout end and every plate splice flagged.
pub fn wall_svg(project: &str, wall: &FramedWall, from: LayoutStart) -> String {
    let heights = if wall.left_height == wall.right_height {
        format!("{} tall", wall.height)
    } else {
        format!(
            "{} tall at the left end, {} at the right",
            wall.left_height, wall.right_height
        )
    };
    let mut doc = Doc::new(&format!("{project}: {} wall", wall.id));
    let area = (
        MARGIN + 40.0,
        MARGIN + 70.0,
        PAGE_W - MARGIN - 40.0,
        PAGE_H - MARGIN - MARK_ROW * MARK_ROWS as f64 - 24.0,
    );
    let f = Frame::fit(
        0.0,
        0.0,
        wall.length.as_inches_f64(),
        wall.height.as_inches_f64(),
        area,
    );
    let mut sub = vec![
        format!("{} long, {heights}, seen from outside", wall.length),
        format!("{}. {}", origin_note(from, "Layout marks"), f.note()),
    ];
    if !wall.splices.is_empty() {
        sub.push("Red lines are plate splices, each centered on a stud.".to_string());
    }
    doc.header(&format!("{}: {} wall", project, wall.id), &sub);

    for o in &wall.openings {
        let r = o.rect;
        doc.polygon(
            &[
                (f.x(r.x), f.y(r.y)),
                (f.x(r.right()), f.y(r.y)),
                (f.x(r.right()), f.y(r.top())),
                (f.x(r.x), f.y(r.top())),
            ],
            "opening",
        );
        let cx = f.x(r.x + half(r.w));
        let cy = f.y(r.y + half(r.h));
        if f.len(r.w) > 40.0 && f.len(r.h) > 24.0 {
            doc.text(cx, cy - 2.0, "dim-text", "middle", &o.name);
            doc.text(
                cx,
                cy + 8.0,
                "sub",
                "middle",
                &format!("{:#} x {:#}", r.w, r.h),
            );
        }
    }
    members(&mut doc, &f, &wall.members, true);
    splices(&mut doc, &f, &wall.members, &wall.splices);
    for s in &wall.splices {
        let y = match s.kind {
            MemberKind::BottomPlate => f.y(Length::ZERO) + 8.0,
            _ => f.y(wall.height) - 4.0,
        };
        doc.text(f.x(s.at), y, "splice-text", "middle", "splice");
    }

    let top = f.y(wall.height) - 20.0;
    doc.dimension(
        (f.x(Length::ZERO), top),
        (f.x(wall.length), top),
        &wall.length.to_string(),
    );
    let (l, r) = (f.x(Length::ZERO) - 18.0, f.x(wall.length) + 18.0);
    doc.dimension(
        (l, f.y(Length::ZERO)),
        (l, f.y(wall.left_height)),
        &wall.left_height.to_string(),
    );
    if wall.right_height != wall.left_height {
        doc.dimension(
            (r, f.y(Length::ZERO)),
            (r, f.y(wall.right_height)),
            &wall.right_height.to_string(),
        );
    }
    marks(
        &mut doc,
        &f,
        &wall.marks,
        from,
        wall.length,
        f.y(Length::ZERO),
    );
    doc.finish()
}

/// The floor framing plan seen from above, front at the bottom: rims,
/// joists, blocking, beams (dashed) and piers, with joist marks and bays.
pub fn floor_svg(project: &str, floor: &FramedFloor, from: LayoutStart) -> String {
    let mut doc = Doc::new(&format!("{project}: floor framing"));
    let area = (
        MARGIN + 40.0,
        MARGIN + 80.0,
        PAGE_W - MARGIN - 40.0,
        PAGE_H - MARGIN - MARK_ROW * 2.0 - 30.0,
    );
    let f = Frame::fit(
        0.0,
        0.0,
        floor.width.as_inches_f64(),
        floor.depth.as_inches_f64(),
        area,
    );
    let mut bays: Vec<(Length, usize)> = Vec::new();
    for b in &floor.bays {
        match bays.iter_mut().find(|(w, _)| *w == b.width) {
            Some((_, n)) => *n += 1,
            None => bays.push((b.width, 1)),
        }
    }
    let bay_list: Vec<String> = bays.iter().map(|(w, n)| format!("{n} at {w:#}")).collect();
    doc.header(
        &format!("{project}: floor framing"),
        &[
            format!(
                "{} wide by {} deep, seen from above with the front at the bottom",
                floor.width, floor.depth
            ),
            format!("Bays between joists (clear): {}", bay_list.join(", ")),
            format!(
                "{}. Beams dashed, piers as circles, splices in red. {}",
                origin_note(from, "Joist marks"),
                f.note()
            ),
        ],
    );
    // In plan the front (y = 0) is at the bottom of the page.
    let beams: Vec<Placed> = floor
        .members
        .iter()
        .filter(|p| matches!(p.member.kind, MemberKind::Beam | MemberKind::Skid))
        .cloned()
        .collect();
    let framing: Vec<Placed> = floor
        .members
        .iter()
        .filter(|p| !matches!(p.member.kind, MemberKind::Beam | MemberKind::Skid))
        .cloned()
        .collect();
    members(&mut doc, &f, &framing, true);
    members(&mut doc, &f, &beams, false);
    for p in &floor.piers {
        doc.circle(f.x(p.x), f.y(p.y), 4.0, "pier");
    }
    splices(&mut doc, &f, &floor.members, &floor.splices);
    let top = f.y(floor.depth) - 14.0;
    doc.dimension(
        (f.x(Length::ZERO), top),
        (f.x(floor.width), top),
        &floor.width.to_string(),
    );
    let left = f.x(Length::ZERO) - 18.0;
    doc.dimension(
        (left, f.y(Length::ZERO)),
        (left, f.y(floor.depth)),
        &floor.depth.to_string(),
    );
    marks(
        &mut doc,
        &f,
        &floor.marks,
        from,
        floor.width,
        f.y(Length::ZERO),
    );
    doc.finish()
}

/// Where each subfloor sheet goes, numbered to match the cut list.
pub fn subfloor_svg(project: &str, floor: &FramedFloor) -> String {
    let mut doc = Doc::new(&format!("{project}: subfloor"));
    let area = (
        MARGIN + 40.0,
        MARGIN + 70.0,
        PAGE_W - MARGIN - 40.0,
        PAGE_H - MARGIN - 30.0,
    );
    let f = Frame::fit(
        0.0,
        0.0,
        floor.width.as_inches_f64(),
        floor.depth.as_inches_f64(),
        area,
    );
    doc.header(
        &format!("{project}: subfloor"),
        &[
            "Sheets run across the joists; every other row starts with a half sheet.".to_string(),
            format!("Seen from above with the front at the bottom. {}", f.note()),
        ],
    );
    for s in &floor.subfloor {
        let r = s.rect;
        doc.polygon(
            &[
                (f.x(r.x), f.y(r.y)),
                (f.x(r.right()), f.y(r.y)),
                (f.x(r.right()), f.y(r.top())),
                (f.x(r.x), f.y(r.top())),
            ],
            "sheet",
        );
        let (cx, cy) = (f.x(r.x + half(r.w)), f.y(r.y + half(r.h)));
        doc.text(cx, cy - 2.0, "dim-text", "middle", short(&s.piece.label));
        doc.text(
            cx,
            cy + 8.0,
            "sub",
            "middle",
            &format!("{:#} x {:#}", r.w, r.h),
        );
    }
    let top = f.y(floor.depth) - 14.0;
    doc.dimension(
        (f.x(Length::ZERO), top),
        (f.x(floor.width), top),
        &floor.width.to_string(),
    );
    doc.finish()
}

/// The roof framing plan seen from above: rafters with their layout marks
/// and the fascia across the tails.
pub fn roof_svg(project: &str, roof: &FramedRoof, from: LayoutStart) -> String {
    let mut doc = Doc::new(&format!("{project}: roof framing"));
    let r = &roof.rafter;
    let y_min = -(r.front_overhang + Length::inches(2));
    let span = r.run + Length::inches(4);
    let area = (
        MARGIN + 40.0,
        MARGIN + 80.0,
        PAGE_W - MARGIN - 40.0,
        PAGE_H - MARGIN - MARK_ROW * 2.0 - 30.0,
    );
    let f = Frame::fit(
        0.0,
        y_min.as_inches_f64(),
        roof.width.as_inches_f64(),
        span.as_inches_f64(),
        area,
    );
    doc.header(
        &format!("{project}: roof framing"),
        &[
            format!(
                "Lean-to at {}, high at the front. {} {} rafters, {} long; see the rafter detail for cuts",
                r.pitch,
                roof.marks.len(),
                r.size,
                r.length
            ),
            format!("Seen from above with the front at the bottom; the walls are dashed. {}", f.note()),
            format!("{}. {}", origin_note(from, "Rafter marks, on the cap plates,"), "Red lines are fascia splices."),
        ],
    );
    // The walls under the roof, outside faces.
    let walls = [
        (Length::ZERO, Length::ZERO, roof.width, Length::ZERO),
        (Length::ZERO, roof.depth, roof.width, roof.depth),
        (Length::ZERO, Length::ZERO, Length::ZERO, roof.depth),
        (roof.width, Length::ZERO, roof.width, roof.depth),
    ];
    for (x1, y1, x2, y2) in walls {
        doc.line(f.x(x1), f.y(y1), f.x(x2), f.y(y2), "opening");
    }
    members(&mut doc, &f, &roof.members, true);
    splices(&mut doc, &f, &roof.members, &roof.splices);
    let top = f.y(y_min + span) - 10.0;
    doc.dimension(
        (f.x(Length::ZERO), top),
        (f.x(roof.width), top),
        &roof.width.to_string(),
    );
    let left = f.x(Length::ZERO) - 18.0;
    doc.dimension(
        (left, f.y(-r.front_overhang)),
        (left, f.y(roof.depth + r.back_overhang)),
        &format!("{} run", r.run),
    );
    marks(&mut doc, &f, &roof.marks, from, roof.width, f.y(y_min));
    doc.finish()
}

/// One rafter in profile with every cut dimensioned, upper (front) end on
/// the left, laid flat as it sits on the sawhorses.
pub fn rafter_svg(project: &str, roof: &FramedRoof) -> String {
    let r = &roof.rafter;
    let mut doc = Doc::new(&format!("{project}: rafter detail"));
    let degrees = format!(
        "{}.{}°",
        r.plumb_tenths_of_degree / 10,
        r.plumb_tenths_of_degree % 10
    );
    doc.header(
        &format!("{project}: rafter detail"),
        &[
            format!(
                "{} {} rafters, all cut alike: {} along either edge at {}",
                roof.marks.len(),
                r.size,
                r.length,
                r.pitch
            ),
            format!(
                "Both ends cut plumb, {degrees} from square. Each birdsmouth has a {:#} level seat and a {:#} plumb heel.",
                r.seat, r.heel
            ),
            format!(
                "Heels are measured along the bottom edge from the upper end. Overhangs: {} front, {} back, level.",
                r.front_overhang, r.back_overhang
            ),
        ],
    );
    let theta = (r.pitch.rise.as_inches_f64() / 12.0).atan();
    let (sin, cos, tan) = (theta.sin(), theta.cos(), theta.tan());
    let len = r.length.as_inches_f64();
    let depth = r.size.actual_width().as_inches_f64();
    // Laid flat, a plumb line leans back by the rafter's depth times the slope.
    let lean = depth * tan;
    let area = (
        MARGIN + 30.0,
        MARGIN + 120.0,
        PAGE_W - MARGIN - 30.0,
        PAGE_H - MARGIN - 110.0,
    );
    let f = Frame::fit(-lean, 0.0, len + lean, depth, area);
    let pt = |u: f64, v: f64| (f.x0 + u * f.scale, f.y0 - v * f.scale);
    let seat = r.seat.as_inches_f64() / cos;
    let heel = r.heel.as_inches_f64();
    let mut outline = vec![pt(0.0, 0.0)];
    for at in [r.front_heel_at, r.back_heel_at] {
        let at = at.as_inches_f64();
        outline.push(pt(at - seat, 0.0));
        outline.push(pt(at - heel * sin, heel * cos));
        outline.push(pt(at, 0.0));
    }
    outline.push(pt(len, 0.0));
    outline.push(pt(len - lean, depth));
    outline.push(pt(-lean, depth));
    doc.polygon(&outline, "m rafter");

    let base = f.y0 + 14.0;
    for (at, name) in [
        (r.front_heel_at, "front heel"),
        (r.back_heel_at, "back heel"),
    ] {
        let (x, _) = pt(at.as_inches_f64(), 0.0);
        doc.line(x, f.y0 + 2.0, x, base + 2.0, "mark");
        doc.text(
            x,
            base + 12.0,
            "dim-text",
            "middle",
            &format!("{name} at {at:#}"),
        );
        doc.text(
            x,
            f.y0 - depth * f.scale - 8.0,
            "sub",
            "middle",
            &format!("seat {:#}, heel {:#}", r.seat, r.heel),
        );
    }
    let (x0, _) = pt(0.0, 0.0);
    let (x1, _) = pt(len, 0.0);
    doc.dimension(
        (x0, base + 34.0),
        (x1, base + 34.0),
        &format!("{} along the bottom edge", r.length),
    );

    // A full-size birdsmouth to trace (both are alike): the bottom of the
    // rafter around the heel, 72 points to the inch.
    let full = 72.0;
    let reach = 1.5;
    let show = (heel * cos + 1.25).min(depth);
    let left = MARGIN + 40.0;
    let bottom = MARGIN + 70.0 + show * full;
    let fp = |u: f64, v: f64| (left + u * full, bottom - v * full);
    let at = reach + seat;
    let end = at + reach;
    doc.polygon(
        &[
            fp(0.0, show),
            fp(0.0, 0.0),
            fp(reach, 0.0),
            fp(at - heel * sin, heel * cos),
            fp(at, 0.0),
            fp(end, 0.0),
            fp(end, show),
        ],
        "m rafter",
    );
    let (sx, sy) = fp(reach, 0.0);
    let (cx, cy) = fp(at - heel * sin, heel * cos);
    let (hx, hy) = fp(at, 0.0);
    doc.text(
        (sx + cx) / 2.0,
        (sy + cy) / 2.0 - 8.0,
        "dim-text",
        "middle",
        &format!("seat {:#}, level", r.seat),
    );
    doc.text(
        hx + 6.0,
        (cy + hy) / 2.0 + 3.0,
        "dim-text",
        "start",
        &format!("heel {:#}, plumb", r.heel),
    );
    doc.text(hx, hy + 12.0, "sub", "middle", "heel mark");
    let (lx, ly) = fp(end + 0.4, show);
    for (i, line) in [
        "Full-size birdsmouth.",
        "Print at 100% and check that",
        "the red bar is 1\" long, then",
        "trace it onto the first rafter",
        "with the heel mark on the",
        "measurement below.",
    ]
    .iter()
    .enumerate()
    {
        doc.text(lx, ly + 8.0 + i as f64 * 11.0, "sub", "start", line);
    }
    doc.line(lx, ly + 80.0, lx + full, ly + 80.0, "splice");
    let (mx, _) = pt(len / 2.0, 0.0);
    doc.text(
        mx,
        f.y0 - depth * f.scale - 22.0,
        "sub",
        "middle",
        &format!(
            "Plumb cut {degrees} at each end; plumb depth {:#}",
            r.plumb_depth
        ),
    );
    doc.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sticks_core::Project;
    use sticks_rules::frame_project;

    #[test]
    fn draws_every_part_of_the_shed() {
        let p = Project::from_toml(include_str!("../../../examples/shed.toml")).unwrap();
        let framed = frame_project(&p).unwrap();
        let front = framed.walls[0].as_ref().unwrap();
        let svg = wall_svg(&p.name, front, p.rules.layout_from);
        assert!(svg.starts_with("<svg"));
        assert!(svg.trim_end().ends_with("</svg>"));
        assert!(svg.contains("Backyard shed: front wall"));
        assert!(svg.contains(NOTICE));
        // Every stud position is printed.
        for m in &front.marks {
            assert!(
                svg.contains(&format!(">{}</text>", esc(&format!("{:#}", m.at)))),
                "{}",
                m.at
            );
        }
        assert_eq!(svg.matches("class=\"splice\"").count(), front.splices.len());

        let left = framed.walls[2].as_ref().unwrap();
        let svg = wall_svg(&p.name, left, p.rules.layout_from);
        assert!(svg.contains("tall at the left end"));

        let floor = framed.floor.as_ref().unwrap().as_ref().unwrap();
        let svg = floor_svg(&p.name, floor, p.rules.layout_from);
        assert!(
            svg.contains(&esc(
                "Bays between joists (clear): 1 at 13-3/4\", 12 at 14-1/2\", 1 at 5-3/4\""
            )),
            "{svg}"
        );
        assert_eq!(svg.matches("class=\"pier\"").count(), floor.piers.len());
        assert!(subfloor_svg(&p.name, floor).contains("SF01"));

        let roof = framed.roof.as_ref().unwrap().as_ref().unwrap();
        assert!(roof_svg(&p.name, roof, p.rules.layout_from).contains("R01"));
        let svg = rafter_svg(&p.name, roof);
        assert!(svg.contains(&esc("front heel at 16\"")), "{svg}");
    }

    #[test]
    fn scale_notes_round_to_architect_scales() {
        let f = Frame {
            scale: 72.0 / 4.0 / 12.0,
            x0: 0.0,
            y0: 0.0,
        };
        assert_eq!(f.note(), "Scale 1/4\" = 1' when printed at 100%");
    }
}
