//! Printable drawings: wall elevations, floor, roof, porch and deck framing
//! plans, the subfloor and sheathing layouts, and rafter details. Each is one US letter page in
//! landscape, laid out in points (1/72").

use std::fmt::Write;

use sticks_core::{
    BeamStyle, Deck, DeckCode, DeckSupport, LayoutStart, Length, MemberKind, RoofKind,
};
use sticks_rules::code::beam_name;
use sticks_rules::{
    FramedDeck, FramedFloor, FramedPorch, FramedRoof, FramedSheathing, FramedWall, LayoutMark,
    Placed, RafterDetail, RafterTop, SheathedFace, Splice,
};

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
const MARK_GAP: f64 = 10.0;
/// Height of a leader's jog over to a mark nudged clear of its neighbor.
const MARK_JOG: f64 = 6.0;

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
.rafter {{ fill: #f3e6c8; }} .fascia {{ fill: #e2c992; }} .ridge {{ fill: #c9a86a; }} .tie {{ fill: #d7d0c0; }}
.post {{ fill: #c9a86a; }} .decking {{ fill: #efe3c6; }}
.hole {{ fill: #fff; stroke: #2a6fb0; stroke-width: 0.6; stroke-dasharray: 3 2; }}
.opening {{ fill: none; stroke: #2a6fb0; stroke-width: 0.75; stroke-dasharray: 3 2; }}
.splice {{ stroke: #c0392b; stroke-width: 1.5; }}
.splice-text {{ fill: #c0392b; font-size: 6px; }}
.dim {{ stroke: #444; stroke-width: 0.5; fill: none; }}
.dim-text {{ font-size: 8px; fill: #222; }}
.mark {{ stroke: #777; stroke-width: 0.4; fill: none; }}
.mark-text {{ font-size: 8px; font-weight: bold; fill: #111; paint-order: stroke; stroke: #fff; stroke-width: 1.5px; stroke-linejoin: round; }}
.label {{ font-size: 4.5px; fill: #555; }}
.title {{ font-size: 14px; font-weight: bold; fill: #111; }}
.sub {{ font-size: 8px; fill: #444; }}
.notice {{ font-size: 7px; fill: #666; }}
.pier {{ fill: #fff; stroke: #222; stroke-width: 0.6; }}
.footing {{ fill: none; stroke: #777; stroke-width: 0.5; stroke-dasharray: 2 2; }}
.concrete {{ fill: #e4e4e0; stroke: #555; stroke-width: 0.6; }}
.grade {{ stroke: #6b5a3c; stroke-width: 1.2; }}
.callout {{ font-size: 7px; fill: #222; }}
.lead {{ stroke: #888; stroke-width: 0.4; fill: none; }}
.cell {{ font-size: 7px; fill: #222; }}
.cell-head {{ font-size: 7px; font-weight: bold; fill: #111; }}
.ok {{ font-size: 7px; font-weight: bold; fill: #2b7a3d; }}
.fail {{ font-size: 7px; font-weight: bold; fill: #b3261e; }}
.rule {{ stroke: #ccc; stroke-width: 0.5; }}
.box {{ fill: #fff8e6; stroke: #c9a86a; stroke-width: 0.8; }}
.box-title {{ font-size: 9px; font-weight: bold; fill: #111; }}
.sheet {{ fill: #f6f1e4; stroke: #222; stroke-width: 0.6; }}
</style>
<rect width="{PAGE_W}" height="{PAGE_H}" fill="#fff"/>
"##,
            esc(title)
        );
        Doc { out }
    }

    fn header(&mut self, title: &str, sub: &[String]) {
        self.header_with(title, sub, &[NOTICE]);
    }

    /// A header with its own notice lines along the foot of the page.
    fn header_with(&mut self, title: &str, sub: &[String], notice: &[&str]) {
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
        for (i, line) in notice.iter().enumerate() {
            let y = PAGE_H - MARGIN + 14.0 - (notice.len() - 1 - i) as f64 * 9.0;
            self.text(MARGIN, y, "notice", "start", line);
        }
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
        MemberKind::Fascia => "fascia",
        MemberKind::Post => "post",
        MemberKind::Ridge => "ridge",
        MemberKind::CollarTie => "tie",
        MemberKind::GableStud => "stud",
        MemberKind::Decking => "decking",
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
/// at the member's near edge. Crowded marks drop to the next row, and marks
/// still too close are nudged apart with a jog in their leader, so every
/// mark is printed and none overlap.
fn marks(
    doc: &mut Doc,
    f: &Frame,
    marks: &[LayoutMark],
    from: LayoutStart,
    run: Length,
    base_y: f64,
) {
    let mut placed: Vec<(f64, &LayoutMark)> = marks
        .iter()
        .map(|m| {
            let edge = match from {
                LayoutStart::Left => m.at,
                LayoutStart::Right => run - m.at,
            };
            (f.x(edge), m)
        })
        .collect();
    placed.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut rows: Vec<Vec<(f64, &LayoutMark)>> = Vec::new();
    for (x, m) in placed {
        let open = rows
            .iter()
            .position(|r| r.last().is_some_and(|&(end, _)| x - end >= MARK_GAP));
        let row = match open {
            Some(row) => row,
            None if rows.len() < MARK_ROWS => {
                rows.push(Vec::new());
                rows.len() - 1
            }
            // Out of rows: share the one whose last mark is farthest away.
            None => (0..rows.len())
                .max_by(|&a, &b| {
                    let end = |r: usize| rows[r].last().map_or(f64::MIN, |p| p.0);
                    (x - end(a)).total_cmp(&(x - end(b)))
                })
                .unwrap_or(0),
        };
        rows[row].push((x, m));
    }
    for (row, list) in rows.iter().enumerate() {
        let xs: Vec<f64> = list.iter().map(|p| p.0).collect();
        let at = spread(&xs, MARK_GAP);
        // Text hangs down from the top of its row; the leader reaches it.
        let ty = base_y + 2.0 + MARK_JOG + MARK_ROW * row as f64;
        for (&(x, m), &lx) in list.iter().zip(&at) {
            doc.line(x, base_y + 2.0, x, ty - MARK_JOG, "mark");
            doc.line(x, ty - MARK_JOG, lx, ty - 1.5, "mark");
            doc.vtext(lx + 2.8, ty, "mark-text", "end", &format!("{:#}", m.at));
        }
    }
}

/// Where to put labels for marks at `xs` (in order across) so they're at
/// least `gap` apart: runs of marks too close spread out, centered on where
/// they are.
fn spread(xs: &[f64], gap: f64) -> Vec<f64> {
    // Each run is its marks and where its first label goes.
    let mut runs: Vec<(Vec<f64>, f64)> = Vec::new();
    for &x in xs {
        let mut run = (vec![x], x);
        while let Some(prev) = runs.last() {
            if prev.1 + prev.0.len() as f64 * gap <= run.1 {
                break;
            }
            let (mut all, _) = runs.pop().unwrap();
            all.extend(run.0);
            let mid = all.iter().sum::<f64>() / all.len() as f64;
            let start = mid - (all.len() - 1) as f64 * gap / 2.0;
            run = (all, start);
        }
        runs.push(run);
    }
    runs.iter()
        .flat_map(|(r, start)| (0..r.len()).map(move |i| start + i as f64 * gap))
        .collect()
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

/// The walls' outside faces in plan, dashed, under a roof or porch plan.
fn footprint(doc: &mut Doc, f: &Frame, width: Length, depth: Length) {
    let walls = [
        (Length::ZERO, Length::ZERO, width, Length::ZERO),
        (Length::ZERO, depth, width, depth),
        (Length::ZERO, Length::ZERO, Length::ZERO, depth),
        (width, Length::ZERO, width, depth),
    ];
    for (x1, y1, x2, y2) in walls {
        doc.line(f.x(x1), f.y(y1), f.x(x2), f.y(y2), "opening");
    }
}

/// The roof framing plan seen from above: rafters with their layout marks,
/// the ridge board and collar ties on a gable, and the fascia across the
/// tails.
pub fn roof_svg(project: &str, roof: &FramedRoof, from: LayoutStart) -> String {
    let mut doc = Doc::new(&format!("{project}: roof framing"));
    let r = &roof.rafters[0];
    let back = roof.rafters.last().unwrap_or(r);
    let (fo, bo) = match roof.kind {
        RoofKind::LeanTo => (r.upper_overhang, r.overhang),
        RoofKind::Gable => (r.overhang, back.overhang),
    };
    let y_min = -(fo + Length::inches(2));
    let span = fo + roof.depth + bo + Length::inches(4);
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
    let pairs = roof.marks.len();
    let first = match roof.kind {
        RoofKind::LeanTo => format!(
            "Lean-to at {}, high at the front. {pairs} {} rafters, {} long; see the rafter detail for cuts",
            r.pitch, r.size, r.length
        ),
        RoofKind::Gable => format!(
            "Gable at {}, ridge along the front. {pairs} pairs of {} rafters on a {} ridge board; see the rafter detail for cuts",
            r.pitch,
            r.size,
            roof.members
                .iter()
                .find(|p| p.member.kind == MemberKind::Ridge)
                .map(|p| p.member.size.to_string())
                .unwrap_or_default()
        ),
    };
    let ties = roof
        .members
        .iter()
        .filter(|p| p.member.kind == MemberKind::CollarTie)
        .count();
    let third = if ties > 0 {
        format!(
            "{}. {ties} collar ties in the upper third. Red lines are splices.",
            origin_note(from, "Rafter marks, on the cap plates,")
        )
    } else {
        format!(
            "{}. Red lines are fascia splices.",
            origin_note(from, "Rafter marks, on the cap plates,")
        )
    };
    doc.header(
        &format!("{project}: roof framing"),
        &[
            first,
            format!(
                "Seen from above with the front at the bottom; the walls are dashed. {}",
                f.note()
            ),
            third,
        ],
    );
    footprint(&mut doc, &f, roof.width, roof.depth);
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
        (left, f.y(-fo)),
        (left, f.y(roof.depth + bo)),
        &format!("{} run", fo + roof.depth + bo),
    );
    marks(&mut doc, &f, &roof.marks, from, roof.width, f.y(y_min));
    doc.finish()
}

/// The porch in plan, seen from above with the front wall at the top:
/// posts, the beam, rafters from the ledger out to the fascia.
pub fn porch_svg(project: &str, porch: &FramedPorch, from: LayoutStart) -> String {
    let mut doc = Doc::new(&format!("{project}: porch framing"));
    let r = &porch.rafter;
    let h = &porch.heights;
    let tail = porch.depth + r.overhang + Length::inches(2);
    let area = (
        MARGIN + 40.0,
        MARGIN + 92.0,
        PAGE_W - MARGIN - 40.0,
        PAGE_H - MARGIN - MARK_ROW * 2.0 - 30.0,
    );
    let f = Frame::fit(
        0.0,
        -tail.as_inches_f64(),
        porch.width.as_inches_f64(),
        (tail + Length::inches(6)).as_inches_f64(),
        area,
    );
    let posts: Vec<&Placed> = porch
        .members
        .iter()
        .filter(|p| p.member.kind == MemberKind::Post)
        .collect();
    let standing = if porch.deck.is_some() {
        "on the deck"
    } else {
        "on piers"
    };
    doc.header(
        &format!("{project}: porch framing"),
        &[
            format!(
                "{} deep across the front. {} {} posts {standing}, {} long, under a {}-ply {} beam",
                porch.depth,
                posts.len(),
                posts.first().map(|p| p.member.size.to_string()).unwrap_or_default(),
                posts.first().map(|p| p.member.length.to_tape_string()).unwrap_or_default(),
                porch
                    .members
                    .iter()
                    .filter(|p| p.member.kind == MemberKind::Beam)
                    .map(|p| p.rect.y)
                    .collect::<std::collections::BTreeSet<_>>()
                    .len(),
                porch
                    .members
                    .iter()
                    .find(|p| p.member.kind == MemberKind::Beam)
                    .map(|p| p.member.size.to_string())
                    .unwrap_or_default(),
            ),
            format!(
                "Ledger top {} above the floor; beam top {}, bottom {}. {} {} rafters at {}, {} long; see the porch rafter detail",
                h.ledger_top, h.beam_top, h.beam_bottom, r.count, r.size, r.pitch, r.length
            ),
            format!(
                "Seen from above with the front wall at the top. {}. {}",
                origin_note(from, "Rafter marks, on the ledger,"),
                f.note()
            ),
        ],
    );
    doc.line(
        f.x(Length::ZERO),
        f.y(Length::ZERO),
        f.x(porch.width),
        f.y(Length::ZERO),
        "opening",
    );
    let framing: Vec<Placed> = porch
        .members
        .iter()
        .filter(|p| p.member.kind != MemberKind::Beam)
        .cloned()
        .collect();
    let beams: Vec<Placed> = porch
        .members
        .iter()
        .filter(|p| p.member.kind == MemberKind::Beam)
        .cloned()
        .collect();
    members(&mut doc, &f, &beams, false);
    members(&mut doc, &f, &framing, true);
    for p in &porch.piers {
        doc.circle(f.x(p.x), f.y(p.y), 4.0, "pier");
    }
    splices(&mut doc, &f, &porch.members, &porch.splices);
    let top = f.y(Length::inches(6)) - 4.0;
    doc.dimension(
        (f.x(Length::ZERO), top),
        (f.x(porch.width), top),
        &porch.width.to_string(),
    );
    let left = f.x(Length::ZERO) - 18.0;
    doc.dimension(
        (left, f.y(Length::ZERO)),
        (left, f.y(-porch.depth)),
        &porch.depth.to_string(),
    );
    marks(&mut doc, &f, &porch.marks, from, porch.width, f.y(-tail));
    doc.finish()
}

/// The porch deck framing in plan: ledger, joists, rim, the beam (dashed)
/// on its piers, with joist marks. The decking is described, not drawn.
pub fn porch_deck_svg(project: &str, porch: &FramedPorch, from: LayoutStart) -> Option<String> {
    let deck = porch.deck.as_ref()?;
    let mut doc = Doc::new(&format!("{project}: porch deck"));
    let area = (
        MARGIN + 40.0,
        MARGIN + 92.0,
        PAGE_W - MARGIN - 40.0,
        PAGE_H - MARGIN - MARK_ROW * 2.0 - 30.0,
    );
    let f = Frame::fit(
        0.0,
        -porch.depth.as_inches_f64(),
        porch.width.as_inches_f64(),
        porch.depth.as_inches_f64(),
        area,
    );
    let boards: Vec<&Placed> = deck
        .members
        .iter()
        .filter(|p| p.member.kind == MemberKind::Decking)
        .collect();
    let board = boards
        .first()
        .map(|p| p.member.size.to_string())
        .unwrap_or_default();
    let mut sub = vec![
        format!(
            "Decking top {} below the subfloor; {} rows of {board} along the front, end joints on joists and staggered",
            -deck.heights.top, deck.rows
        ),
        format!(
            "Joists hang from a ledger on the floor's front rim; the beam (dashed) sits on {} piers under the posts. {}",
            deck.piers.len(),
            f.note()
        ),
        format!(
            "{}. Seen from above with the shed at the top.",
            origin_note(from, "Joist marks, on the ledger and rim,")
        ),
    ];
    sub.extend(deck.warnings.iter().map(|w| format!("Note: {w}")));
    doc.header(&format!("{project}: porch deck"), &sub);
    let framing: Vec<Placed> = deck
        .members
        .iter()
        .filter(|p| !matches!(p.member.kind, MemberKind::Beam | MemberKind::Decking))
        .cloned()
        .collect();
    let beams: Vec<Placed> = deck
        .members
        .iter()
        .filter(|p| p.member.kind == MemberKind::Beam)
        .cloned()
        .collect();
    members(&mut doc, &f, &framing, true);
    members(&mut doc, &f, &beams, false);
    for p in &deck.piers {
        doc.circle(f.x(p.x), f.y(p.y), 4.0, "pier");
    }
    let structural: Vec<Placed> = deck
        .members
        .iter()
        .filter(|p| p.member.kind != MemberKind::Decking)
        .cloned()
        .collect();
    splices(&mut doc, &f, &structural, &deck.splices);
    let top = f.y(Length::ZERO) - 14.0;
    doc.dimension(
        (f.x(Length::ZERO), top),
        (f.x(porch.width), top),
        &porch.width.to_string(),
    );
    marks(
        &mut doc,
        &f,
        &deck.marks,
        from,
        porch.width,
        f.y(-porch.depth),
    );
    Some(doc.finish())
}

/// Sheathing panels on one face, numbered to match the cut list.
fn sheathing_face(doc: &mut Doc, f: &Frame, face: &SheathedFace) {
    for p in &face.panels {
        let pts: Vec<(f64, f64)> = if p.outline.is_empty() {
            let r = p.rect;
            vec![
                (f.x(r.x), f.y(r.y)),
                (f.x(r.right()), f.y(r.y)),
                (f.x(r.right()), f.y(r.top())),
                (f.x(r.x), f.y(r.top())),
            ]
        } else {
            p.outline.iter().map(|&(x, y)| (f.x(x), f.y(y))).collect()
        };
        doc.polygon(&pts, "sheet");
        for h in &p.holes {
            doc.polygon(
                &[
                    (f.x(h.x), f.y(h.y)),
                    (f.x(h.right()), f.y(h.y)),
                    (f.x(h.right()), f.y(h.top())),
                    (f.x(h.x), f.y(h.top())),
                ],
                "hole",
            );
        }
        let r = p.rect;
        if f.len(r.w) > 16.0 && f.len(r.h) > 14.0 {
            let (cx, cy) = (f.x(r.x + half(r.w)), f.y(r.y + half(r.h)));
            doc.text(cx, cy - 1.0, "label", "middle", short(&p.piece.label));
            if f.len(r.w) > 30.0 {
                doc.text(
                    cx,
                    cy + 5.0,
                    "label",
                    "middle",
                    &format!("{:#} x {:#}", r.w, r.h),
                );
            }
        }
    }
}

/// Every face's sheathing on one page: the walls seen from outside in a
/// grid, or the roof planes laid flat, eaves at the bottom.
pub fn sheathing_svg(project: &str, sheathing: &FramedSheathing, roofs: bool) -> Option<String> {
    let faces = if roofs {
        &sheathing.roofs
    } else {
        &sheathing.walls
    };
    let first = faces.first()?;
    let what = if roofs {
        "roof sheathing"
    } else {
        "wall sheathing"
    };
    let mut doc = Doc::new(&format!("{project}: {what}"));
    let pieces: usize = faces.iter().map(|f| f.panels.len()).sum();
    doc.header(
        &format!("{project}: {what}"),
        &[
            format!(
                "{pieces} pieces of {:#} sheathing, numbered to match the cut list",
                first.thickness
            ),
            if roofs {
                "Each roof face laid flat with its eave at the bottom; sheets run across the rafters, rows staggered by half a sheet.".to_string()
            } else {
                "Each wall seen from outside. Sheets stand on end with joints on studs; doors and windows (dashed) are cut out.".to_string()
            },
        ],
    );
    let cols = if faces.len() > 1 { 2 } else { 1 };
    let rows = faces.len().div_ceil(cols);
    let (x0, y0) = (MARGIN, MARGIN + 60.0);
    let cell_w = (PAGE_W - MARGIN * 2.0) / cols as f64;
    let cell_h = (PAGE_H - MARGIN - 24.0 - y0) / rows as f64;
    for (i, face) in faces.iter().enumerate() {
        let (c, r) = (i % cols, i / cols);
        let left = x0 + c as f64 * cell_w;
        let top = y0 + r as f64 * cell_h;
        let xs = face.outline.iter().map(|p| p.0.as_inches_f64());
        let ys = face.outline.iter().map(|p| p.1.as_inches_f64());
        let (min_x, max_x) = xs.fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(v), b.max(v)));
        let ys: Vec<f64> = ys.collect();
        let min_y = ys.iter().copied().fold(f64::MAX, f64::min);
        let max_y = ys.iter().copied().fold(f64::MIN, f64::max);
        let f = Frame::fit(
            min_x,
            min_y,
            max_x - min_x,
            max_y - min_y,
            (
                left + 12.0,
                top + 16.0,
                left + cell_w - 12.0,
                top + cell_h - 8.0,
            ),
        );
        let title = if roofs {
            face.name.clone()
        } else {
            format!("{} wall", face.name)
        };
        doc.text(left + 12.0, top + 10.0, "dim-text", "start", &title);
        sheathing_face(&mut doc, &f, face);
    }
    Some(doc.finish())
}

/// One rafter in profile with every cut dimensioned, upper end on the
/// left, laid flat as it sits on the sawhorses.
pub fn rafter_svg(project: &str, r: &RafterDetail) -> String {
    let title = format!("{project}: {} detail", r.name);
    let mut doc = Doc::new(&title);
    let degrees = format!(
        "{}.{}°",
        r.plumb_tenths_of_degree / 10,
        r.plumb_tenths_of_degree % 10
    );
    let upper = match r.top {
        RafterTop::Tail => format!("The upper end overhangs {} level", r.upper_overhang),
        RafterTop::Ridge => "The upper end butts the ridge board".to_string(),
        RafterTop::Ledger => "The upper end hangs on the ledger".to_string(),
    };
    let each = if r.heels.len() == 1 {
        "The birdsmouth has"
    } else {
        "Each birdsmouth has"
    };
    doc.header(
        &title,
        &[
            format!(
                "{} {} {}s, all cut alike: {} along either edge at {}",
                r.count, r.size, r.name, r.length, r.pitch
            ),
            format!(
                "Both ends cut plumb, {degrees} from square. {each} a {:#} level seat and a {:#} plumb heel.",
                r.seat, r.heel
            ),
            format!(
                "Heels are measured along the bottom edge from the upper end. {upper}; the tail {} past the last seat.",
                r.overhang
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
    for &at in &r.heels {
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
    let names: &[&str] = if r.heels.len() == 2 {
        &["front heel", "back heel"]
    } else {
        &["heel"]
    };
    for (&at, name) in r.heels.iter().zip(names) {
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

/// The line every deck sheet carries: what the sizes were checked against,
/// and that the builder owns the plans.
pub fn deck_notice(deck: &Deck) -> [String; 2] {
    [
        format!(
            "Sizes checked by Sticks against {}. Sticks is not an engineer or a building official.",
            deck.code.name()
        ),
        "You are responsible for these plans: confirm every size, footing and connector with your building department before you build."
            .to_string(),
    ]
}

fn deck_header(doc: &mut Doc, deck: &Deck, title: &str, sub: &[String]) {
    let notice = deck_notice(deck);
    doc.header_with(title, sub, &[&notice[0], &notice[1]]);
}

/// A chain of dimensions between `stops` along `x`, at height `y` on the page.
fn x_chain(doc: &mut Doc, f: &Frame, stops: &[Length], y: f64) {
    for w in stops.windows(2) {
        if w[1] > w[0] {
            doc.dimension(
                (f.x(w[0]), y),
                (f.x(w[1]), y),
                &format!("{:#}", w[1] - w[0]),
            );
        }
    }
}

/// A chain of dimensions between `stops` along `y`, at `x` on the page.
fn y_chain(doc: &mut Doc, f: &Frame, stops: &[Length], x: f64) {
    for w in stops.windows(2) {
        if w[1] > w[0] {
            doc.dimension(
                (x, f.y(w[0])),
                (x, f.y(w[1])),
                &format!("{:#}", w[1] - w[0]),
            );
        }
    }
}

fn of_kind(members: &[Placed], kinds: &[MemberKind]) -> Vec<Placed> {
    members
        .iter()
        .filter(|p| kinds.contains(&p.member.kind))
        .cloned()
        .collect()
}

/// How the deck stands, for sheet notes: `4x4 posts 10-1/2" long on 16" round footings`.
fn standing(d: &FramedDeck) -> String {
    let footing = d
        .footings
        .iter()
        .max_by_key(|f| f.diameter)
        .map(|f| {
            let kind = f.kind.map(|k| format!("type {k}, ")).unwrap_or_default();
            format!(
                "{kind}{:#} round footings {:#} thick, bottoms {:#} below grade",
                f.diameter, f.thickness, f.depth
            )
        })
        .unwrap_or_default();
    let post = d
        .members
        .iter()
        .find(|p| p.member.kind == MemberKind::Post)
        .map(|p| {
            format!(
                "{} posts {} long",
                p.member.size,
                p.member.length.to_tape_string()
            )
        });
    match (d.support, post) {
        (DeckSupport::Blocks, Some(p)) => format!("{p} in precast deck blocks"),
        (_, Some(p)) => format!("{p} on {footing}"),
        _ => format!("beams on concrete piers: {footing}"),
    }
}

/// The deck's framing plan seen from above, front at the bottom: joists,
/// rims and blocking, beams dashed, posts, piers and the footings around
/// them, with the beam lines and supports dimensioned.
pub fn deck_plan_svg(project: &str, deck: &Deck, d: &FramedDeck, from: LayoutStart) -> String {
    let title = format!("{project}: deck framing plan");
    let mut doc = Doc::new(&title);
    let area = (
        MARGIN + 50.0,
        MARGIN + 100.0,
        PAGE_W - MARGIN - 90.0,
        PAGE_H - MARGIN - MARK_ROW * 2.0 - 40.0,
    );
    let f = Frame::fit(
        0.0,
        0.0,
        d.width.as_inches_f64(),
        d.depth.as_inches_f64(),
        area,
    );
    let joist = deck.joist;
    let lines = d.beam_y.len();
    doc.header_with(
        &title,
        &[
            format!(
                "{} wide by {} deep, decking {} above grade. {joist} joists at {:#} on center {} {lines} beams of {}",
                d.width,
                d.depth,
                d.heights.top,
                d.layout.oc,
                if d.beam_style == BeamStyle::Flush {
                    "hung between"
                } else {
                    "on"
                },
                beam_name(d.beam, d.beam_plies)
            ),
            format!("Beams on {}", standing(d)),
            format!(
                "Seen from above with the front at the bottom. Beams dashed, footings as dashed circles. {}. {}",
                origin_note(from, "Joist marks"),
                f.note()
            ),
        ],
        &[&deck_notice(deck)[0], &deck_notice(deck)[1]],
    );
    let framing = of_kind(
        &d.members,
        &[
            MemberKind::Joist,
            MemberKind::RimJoist,
            MemberKind::Blocking,
        ],
    );
    let beams = of_kind(&d.members, &[MemberKind::Beam]);
    let posts = of_kind(&d.members, &[MemberKind::Post]);
    members(&mut doc, &f, &framing, true);
    members(&mut doc, &f, &beams, false);
    members(&mut doc, &f, &posts, false);
    for (p, footing) in d.piers.iter().zip(&d.footings) {
        doc.circle(f.x(p.x), f.y(p.y), f.len(half(footing.diameter)), "footing");
        if posts.is_empty() {
            doc.circle(f.x(p.x), f.y(p.y), 3.0, "pier");
        }
    }
    let structural = of_kind(
        &d.members,
        &[MemberKind::Joist, MemberKind::RimJoist, MemberKind::Beam],
    );
    splices(&mut doc, &f, &structural, &d.splices);

    // Width over the top, the supports along a beam under it.
    let top = f.y(d.depth) - 14.0;
    doc.dimension(
        (f.x(Length::ZERO), top),
        (f.x(d.width), top),
        &d.width.to_string(),
    );
    let mut stops = vec![Length::ZERO];
    stops.extend(d.support_x.iter().copied());
    stops.push(d.width);
    x_chain(&mut doc, &f, &stops, top - 16.0);
    doc.text(
        f.x(Length::ZERO) - 4.0,
        top - 22.0,
        "sub",
        "end",
        "post centers",
    );
    // Depth on the left, beam centers on the right.
    let left = f.x(Length::ZERO) - 18.0;
    doc.dimension(
        (left, f.y(Length::ZERO)),
        (left, f.y(d.depth)),
        &d.depth.to_string(),
    );
    let mut stops = vec![Length::ZERO];
    stops.extend(d.beam_y.iter().copied());
    stops.push(d.depth);
    let right = f.x(d.width) + 20.0;
    y_chain(&mut doc, &f, &stops, right);
    doc.text(
        right + 6.0,
        f.y(d.depth) - 8.0,
        "sub",
        "start",
        "beam centers",
    );
    marks(&mut doc, &f, &d.marks, from, d.width, f.y(Length::ZERO));
    doc.finish()
}

/// A section through the deck along the joists at a line of posts: grade,
/// footings, posts, beams, a joist and the decking, with heights and the
/// connections called out.
pub fn deck_section_svg(project: &str, deck: &Deck, d: &FramedDeck) -> String {
    let title = format!("{project}: deck section");
    let mut doc = Doc::new(&title);
    let h = d.heights;
    let deepest = d
        .footings
        .iter()
        .map(|f| f.depth)
        .max()
        .unwrap_or(Length::inches(12));
    let widest = d
        .footings
        .iter()
        .map(|f| f.diameter)
        .max()
        .unwrap_or(Length::inches(12));
    let below = deepest + Length::inches(4);
    let area = (
        MARGIN + 60.0,
        MARGIN + 80.0,
        PAGE_W - MARGIN - 60.0,
        PAGE_H - MARGIN - 175.0,
    );
    let f = Frame::fit(
        -12.0,
        -below.as_inches_f64(),
        (d.depth + Length::inches(24)).as_inches_f64(),
        (h.top + below + Length::inches(4)).as_inches_f64(),
        area,
    );
    deck_header(
        &mut doc,
        deck,
        &title,
        &[
            format!(
                "Cut along the joists through a line of posts, front at the left. {}",
                f.note()
            ),
            format!("Beams on {}", standing(d)),
        ],
    );
    let rect = |doc: &mut Doc, x: Length, z: Length, w: Length, hgt: Length, class: &str| {
        doc.polygon(
            &[
                (f.x(x), f.y(z)),
                (f.x(x + w), f.y(z)),
                (f.x(x + w), f.y(z + hgt)),
                (f.x(x), f.y(z + hgt)),
            ],
            class,
        );
    };
    // Grade.
    doc.line(
        f.x(Length::inches(-12)),
        f.y(Length::ZERO),
        f.x(d.depth + Length::inches(12)),
        f.y(Length::ZERO),
        "grade",
    );
    doc.text(
        f.x(Length::inches(-12)),
        f.y(Length::ZERO) + 10.0,
        "sub",
        "start",
        "grade",
    );
    let joist = deck.joist;
    let t = joist.actual_thickness();
    let bw = d.beam.actual_thickness() * i64::from(d.beam_plies);
    let post = d.members.iter().find(|p| p.member.kind == MemberKind::Post);
    for &y in &d.beam_y {
        // Concrete from the footing's bottom up to the pier top.
        let fw = widest;
        rect(
            &mut doc,
            y - half(fw),
            -deepest,
            fw,
            deepest + h.pier_top,
            "concrete",
        );
        if let Some(p) = post {
            let pw = p.member.size.actual_width();
            rect(
                &mut doc,
                y - half(pw),
                h.pier_top,
                pw,
                h.beam_bottom - h.pier_top,
                "m post",
            );
        }
        rect(
            &mut doc,
            y - half(bw),
            h.beam_bottom,
            bw,
            h.beam_top - h.beam_bottom,
            "m header",
        );
    }
    // A joist, with the rims at its ends when it sits on the beams.
    let jh = h.joist_top - h.joist_bottom;
    if d.beam_style == BeamStyle::Flush {
        for w in d.beam_y.windows(2) {
            rect(
                &mut doc,
                w[0] + half(bw),
                h.joist_bottom,
                w[1] - w[0] - bw,
                jh,
                "m joist",
            );
        }
    } else {
        rect(&mut doc, Length::ZERO, h.joist_bottom, t, jh, "m rim");
        rect(&mut doc, t, h.joist_bottom, d.depth - t * 2, jh, "m joist");
        rect(&mut doc, d.depth - t, h.joist_bottom, t, jh, "m rim");
    }
    rect(
        &mut doc,
        Length::ZERO,
        h.joist_top,
        d.depth,
        h.top - h.joist_top,
        "m decking",
    );

    // What each piece is, beside the first post line.
    let y = d.beam_y[0];
    let label = |doc: &mut Doc, x: Length, z: Length, text: &str| {
        doc.text(f.x(x), f.y(z) + 2.5, "callout", "start", text);
    };
    label(
        &mut doc,
        y + half(widest) + Length::inches(2),
        -half(deepest),
        "concrete footing",
    );
    if let Some(p) = post {
        label(
            &mut doc,
            y + half(p.member.size.actual_width()) + Length::inches(2),
            h.pier_top + half(h.beam_bottom - h.pier_top),
            &format!("{} post", p.member.size),
        );
    }
    let beam_label = format!("{} beam", beam_name(d.beam, d.beam_plies));
    let beam_mid = h.beam_bottom + half(h.beam_top - h.beam_bottom);
    if d.beam_style == BeamStyle::Flush {
        label(
            &mut doc,
            y + half(bw) + Length::inches(18),
            h.joist_bottom - Length::inches(3),
            &beam_label,
        );
    } else {
        label(
            &mut doc,
            y + half(bw) + Length::inches(2),
            beam_mid,
            &beam_label,
        );
    }
    label(
        &mut doc,
        d.depth.mul_div(1, 2) - Length::inches(12),
        h.joist_bottom + half(h.joist_top - h.joist_bottom),
        &format!("{joist} joist"),
    );
    doc.text(
        f.x(d.depth.mul_div(1, 2)),
        f.y(h.top) - 4.0,
        "callout",
        "middle",
        &format!("{} decking", deck.decking),
    );

    // Heights on the left.
    let left = f.x(Length::ZERO) - 26.0;
    doc.dimension(
        (left, f.y(Length::ZERO)),
        (left, f.y(h.top)),
        &format!("{:#} to top of decking", h.top),
    );
    doc.dimension(
        (left, f.y(-deepest)),
        (left, f.y(Length::ZERO)),
        &format!("{:#}", deepest),
    );
    // Beam centers along the bottom.
    let mut stops = vec![Length::ZERO];
    stops.extend(d.beam_y.iter().copied());
    stops.push(d.depth);
    x_chain(&mut doc, &f, &stops, f.y(-below) + 14.0);

    // Callouts under the drawing.
    let mut notes = vec![
        format!(
            "Decking: {} boards, (2) 8d nails or (2) #8 screws at every joist, 1/8\" gaps",
            deck.decking
        ),
        if d.beam_style == BeamStyle::Flush {
            format!(
                "Joists: {joist} at {:#} o.c. in face-mount hangers at least 60% of the joist depth",
                d.layout.oc
            )
        } else {
            format!(
                "Joists: {joist} at {:#} o.c., a hurricane tie (100 lb uplift and lateral) at every joist on a beam",
                d.layout.oc
            )
        },
        format!(
            "Beams: {}, plies joined with 10d nails or #10 x 3\" screws in 2 staggered rows at 16\" o.c., 2 at each end and splice",
            beam_name(d.beam, d.beam_plies)
        ),
    ];
    if let Some(p) = post {
        notes.push(format!(
            "Posts: {} {} long, a post cap sized for the beam on top and a post base with a 1\" standoff below",
            p.member.size,
            p.member.length.to_tape_string()
        ));
    }
    if let Some(ft) = d.footings.iter().max_by_key(|f| f.diameter) {
        let kind = ft.kind.map(|k| format!("type {k}, ")).unwrap_or_default();
        notes.push(format!(
            "Footings: {kind}{:#} round or {:#} square, at least {:#} thick, bottom {:#} below grade on undisturbed soil",
            ft.diameter, ft.side, ft.thickness, ft.depth
        ));
    }
    let y0 = PAGE_H - MARGIN - 105.0;
    for (i, n) in notes.iter().enumerate() {
        doc.text(MARGIN, y0 + i as f64 * 11.0, "callout", "start", n);
    }
    doc.finish()
}

/// Decking boards in plan, numbered to match the cut list.
pub fn decking_svg(project: &str, deck: &Deck, d: &FramedDeck) -> String {
    let title = format!("{project}: decking");
    let mut doc = Doc::new(&title);
    let area = (
        MARGIN + 40.0,
        MARGIN + 70.0,
        PAGE_W - MARGIN - 40.0,
        PAGE_H - MARGIN - 40.0,
    );
    let f = Frame::fit(
        0.0,
        0.0,
        d.width.as_inches_f64(),
        d.depth.as_inches_f64(),
        area,
    );
    deck_header(
        &mut doc,
        deck,
        &title,
        &[
            format!(
                "{} rows of {} across the joists, a full board at the front, end joints on joists and staggered row to row",
                d.rows, deck.decking
            ),
            format!("Seen from above with the front at the bottom. {}", f.note()),
        ],
    );
    let boards = of_kind(&d.members, &[MemberKind::Decking]);
    members(&mut doc, &f, &boards, true);
    let top = f.y(d.depth) - 14.0;
    doc.dimension(
        (f.x(Length::ZERO), top),
        (f.x(d.width), top),
        &d.width.to_string(),
    );
    doc.finish()
}

/// Splits `text` into lines of at most `width` characters at spaces.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        if !line.is_empty() && line.len() + 1 + word.len() > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// General notes for a deck under its rules.
pub fn deck_general_notes(deck: &Deck) -> Vec<String> {
    match deck.code {
        DeckCode::Fairfax2021 => vec![
            "Lumber: preservative-treated southern pine, No. 2 or better. Members touching the ground rated for ground contact (UC4A). Field-treat cut post ends with copper naphthenate.".to_string(),
            "Fasteners hot-dipped galvanized or stainless; connectors G-185 (ZMAX or equal) or stainless, installed with the maker's fasteners.".to_string(),
            "Footings bear on solid ground 24\" below grade, deeper where solid ground isn't found; 3,000 psi concrete. Footings within 5' of the house bear as deep as the house footings.".to_string(),
            "Call Miss Utility (811) before digging. Inspections: footing (holes dug, before concrete), framing (before decking), final.".to_string(),
            "Keep the approved plat, these plans and the county's Typical Deck Details on site.".to_string(),
        ],
        DeckCode::Irc2021 => vec![
            "Lumber: preservative-treated or naturally durable, No. 2 or better (IRC R507.2). Members touching the ground rated for ground contact.".to_string(),
            "Fasteners and connectors hot-dipped galvanized or stainless per IRC Table R507.2.3, installed with the maker's fasteners.".to_string(),
            "Footings at least 12\" below undisturbed ground and below the frost line your building department sets (IRC R507.3).".to_string(),
            "Call 811 before digging. Ask your building department which inspections it needs; usually footing, framing and final.".to_string(),
        ],
    }
}

/// Every check the deck was put through, with where each limit comes from,
/// the general notes, and a box for the builder to sign that they own the
/// plans.
pub fn deck_checks_svg(project: &str, deck: &Deck, d: &FramedDeck) -> String {
    let title = format!("{project}: code checks");
    let mut doc = Doc::new(&title);
    let passed = d.checks.iter().filter(|c| c.ok).count();
    deck_header(
        &mut doc,
        deck,
        &title,
        &[format!(
            "{} of {} checks pass against {}. Live load {} psf, dead load 10 psf.",
            passed,
            d.checks.len(),
            deck.code.name(),
            deck.live_psf
        )],
    );
    // Columns: result, item, design, allowed, source.
    let cols = [
        (MARGIN, 28.0, ""),
        (MARGIN + 28.0, 120.0, "Check"),
        (MARGIN + 148.0, 170.0, "This deck"),
        (MARGIN + 318.0, 210.0, "Allowed or required"),
        (MARGIN + 528.0, 192.0, "Source"),
    ];
    let mut y = MARGIN + 52.0;
    for (x, _, head) in cols {
        doc.text(x, y, "cell-head", "start", head);
    }
    y += 4.0;
    doc.line(MARGIN, y, PAGE_W - MARGIN, y, "rule");
    y += 9.0;
    for c in &d.checks {
        let cells = [&c.item, &c.actual, &c.allowed, &c.source];
        let wrapped: Vec<Vec<String>> = cells
            .iter()
            .zip(&cols[1..])
            .map(|(text, (_, w, _))| wrap(text, (w / 3.4) as usize))
            .collect();
        let rows = wrapped.iter().map(Vec::len).max().unwrap_or(1);
        doc.text(
            cols[0].0,
            y,
            if c.ok { "ok" } else { "fail" },
            "start",
            if c.ok { "OK" } else { "FAIL" },
        );
        for (lines, (x, _, _)) in wrapped.iter().zip(&cols[1..]) {
            for (i, line) in lines.iter().enumerate() {
                doc.text(*x, y + i as f64 * 8.5, "cell", "start", line);
            }
        }
        y += rows as f64 * 8.5 + 2.0;
        doc.line(MARGIN, y - 6.0, PAGE_W - MARGIN, y - 6.0, "rule");
        y += 3.0;
    }
    y += 8.0;
    doc.text(MARGIN, y, "cell-head", "start", "General notes");
    y += 10.0;
    for note in deck_general_notes(deck) {
        for (i, line) in wrap(&note, 190).iter().enumerate() {
            let text = if i == 0 {
                format!("- {line}")
            } else {
                format!("  {line}")
            };
            doc.text(MARGIN, y, "cell", "start", &text);
            y += 8.5;
        }
    }
    // The builder's acknowledgement.
    let box_y = (y + 8.0).min(PAGE_H - MARGIN - 70.0);
    doc.polygon(
        &[
            (MARGIN, box_y),
            (PAGE_W - MARGIN, box_y),
            (PAGE_W - MARGIN, box_y + 46.0),
            (MARGIN, box_y + 46.0),
        ],
        "box",
    );
    doc.text(
        MARGIN + 8.0,
        box_y + 13.0,
        "box-title",
        "start",
        "The builder is responsible for these plans",
    );
    doc.text(
        MARGIN + 8.0,
        box_y + 24.0,
        "cell",
        "start",
        "Sticks drew these plans from the sizes entered and checked them against the tables above. It is not an engineer, and passing these checks is not a permit.",
    );
    doc.text(
        MARGIN + 8.0,
        box_y + 38.0,
        "cell",
        "start",
        "I have reviewed these plans and will confirm them with my building department.   Signed ______________________________   Date ____________",
    );
    doc.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sticks_core::Project;
    use sticks_rules::frame_project;

    #[test]
    fn crowded_marks_spread_apart_around_where_they_are() {
        // A king and jack 1-1/2" apart at a small scale, then one in the clear.
        let at = spread(&[100.0, 103.0, 104.0, 200.0], 10.0);
        assert_eq!(
            at,
            vec![
                92.33333333333333,
                102.33333333333333,
                112.33333333333333,
                200.0
            ]
        );
        let at = spread(&[0.0, 20.0, 40.0], 10.0);
        assert_eq!(at, vec![0.0, 20.0, 40.0]);
    }

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
        assert_eq!(
            svg.matches("class=\"mark-text\"").count(),
            front.marks.len()
        );
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
        let svg = rafter_svg(&p.name, &roof.rafters[0]);
        assert!(svg.contains(&esc("front heel at 15-11/16\"")), "{svg}");

        let porch = framed.porch.as_ref().unwrap().as_ref().unwrap();
        let svg = porch_svg(&p.name, porch, p.rules.layout_from);
        assert!(
            svg.contains("porch framing") && svg.contains(">P1<"),
            "{svg}"
        );
        assert!(rafter_svg(&p.name, &porch.rafter).contains("porch rafter"));
        let svg = porch_deck_svg(&p.name, porch, p.rules.layout_from).unwrap();
        assert_eq!(svg.matches("class=\"pier\"").count(), 4);

        let sheathing = framed.sheathing.as_ref().unwrap().as_ref().unwrap();
        let svg = sheathing_svg(&p.name, sheathing, false).unwrap();
        assert!(
            svg.contains("front-SH01") || svg.contains(">SH01<"),
            "{svg}"
        );
        assert!(svg.contains("class=\"hole\""));
        assert!(sheathing_svg(&p.name, sheathing, true)
            .unwrap()
            .contains("porch roof"));
    }

    #[test]
    fn draws_the_example_deck_with_its_checks_and_notice() {
        let p =
            sticks_core::Project::from_toml(include_str!("../../../examples/deck.toml")).unwrap();
        let framed = sticks_rules::frame_project(&p).unwrap();
        let d = framed.deck.as_ref().unwrap().as_ref().unwrap();
        let deck = p.deck.as_ref().unwrap();
        let checks = deck_checks_svg(&p.name, deck, d);
        assert_eq!(checks.matches("class=\"ok\"").count(), d.checks.len());
        assert!(checks.contains("The builder is responsible for these plans"));
        assert!(checks.contains("Fairfax Table 2"));
        let plan = deck_plan_svg(&p.name, deck, d, p.rules.layout_from);
        assert!(plan.contains("You are responsible for these plans"));
        assert_eq!(plan.matches("class=\"footing\"").count(), d.piers.len());
        let section = deck_section_svg(&p.name, deck, d);
        assert!(section.contains("concrete footing") && section.contains("hurricane tie"));
        assert!(decking_svg(&p.name, deck, d).contains("DK01"));
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
