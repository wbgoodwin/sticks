//! Plain-text output for a terminal or a printout.

use std::fmt::Write;

use sticks_core::{Cut, LayoutStart};
use sticks_cutlist::CutList;
use sticks_rules::{FramedFloor, FramedRoof, FramedWall, LayoutMark, Placed, Splice};

fn members(out: &mut String, members: &[Placed]) {
    let _ = writeln!(
        out,
        "  {:<14} {:<13} {:<5} {:>10}  cuts",
        "label", "member", "size", "length"
    );
    for p in members {
        let m = &p.member;
        let cuts: Vec<String> = m
            .cuts
            .iter()
            .filter(|c| **c != Cut::Square)
            .map(Cut::describe)
            .collect();
        let _ = writeln!(
            out,
            "  {:<14} {:<13} {:<5} {:>10}  {}",
            m.label,
            m.kind.name(),
            m.size.to_string(),
            m.length.to_tape_string(),
            cuts.join("; ")
        );
    }
    let _ = writeln!(out);
}

fn marks(out: &mut String, marks: &[LayoutMark], from: LayoutStart, what: &str) {
    let end = match from {
        LayoutStart::Left => "left",
        LayoutStart::Right => "right",
    };
    let _ = writeln!(out, "  {what}, measured from the {end} end:");
    for m in marks {
        let _ = writeln!(
            out,
            "  {:>10}  {} ({})",
            format!("{:#}", m.at),
            m.label,
            m.kind.name()
        );
    }
}

fn splices(out: &mut String, splices: &[Splice]) {
    for s in splices {
        let _ = writeln!(
            out,
            "  splice: {} joint at {:#} from the left end",
            s.kind.name(),
            s.at
        );
    }
}

fn notes(out: &mut String, warnings: &[String]) {
    for warning in warnings {
        let _ = writeln!(out, "  note: {warning}");
    }
    let _ = writeln!(out);
}

pub fn wall(w: &FramedWall, from: LayoutStart) -> String {
    let mut out = String::new();
    let height = if w.left_height == w.right_height {
        format!("{} tall", w.height)
    } else {
        format!(
            "{} tall at the left end, {} at the right",
            w.left_height, w.right_height
        )
    };
    let _ = writeln!(out, "Wall {}: {} long, {height}", w.id, w.length);
    let _ = writeln!(out);
    members(&mut out, &w.members);
    marks(&mut out, &w.marks, from, "Layout marks");
    splices(&mut out, &w.splices);
    notes(&mut out, &w.warnings);
    out
}

pub fn floor(f: &FramedFloor, from: LayoutStart) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "Floor: {} wide, {} deep", f.width, f.depth);
    let _ = writeln!(out);
    members(&mut out, &f.members);
    marks(&mut out, &f.marks, from, "Joist marks on the rims");
    let _ = writeln!(out, "  Bays between joists, clear, from the left end:");
    for b in &f.bays {
        let _ = writeln!(out, "  {:>10}  at {:#}", format!("{:#}", b.width), b.x);
    }
    let _ = writeln!(out, "  {} piers, centered under the beams:", f.piers.len());
    for p in &f.piers {
        let _ = writeln!(
            out,
            "  {:>10} from the left, {:#} from the front",
            format!("{:#}", p.x),
            p.y
        );
    }
    let _ = writeln!(out, "  Subfloor, {} pieces:", f.subfloor.len());
    for s in &f.subfloor {
        let _ = writeln!(
            out,
            "  {:<14} {:#} x {:#} at {:#}, {:#}",
            s.piece.label, s.rect.w, s.rect.h, s.rect.x, s.rect.y
        );
    }
    splices(&mut out, &f.splices);
    notes(&mut out, &f.warnings);
    out
}

pub fn roof(r: &FramedRoof, from: LayoutStart) -> String {
    let mut out = String::new();
    let d = &r.rafter;
    let _ = writeln!(
        out,
        "Roof: lean-to at {}, {} wide, high at the front",
        d.pitch, r.width
    );
    let _ = writeln!(
        out,
        "  Walls: front {}, back {}; side walls rake from {} to {}",
        r.heights.front, r.heights.back, r.heights.side_front, r.heights.side_back
    );
    let _ = writeln!(
        out,
        "  Rafters: {} {}, {} plumb cut to plumb cut, plumb cuts {}.{}° from square",
        r.marks.len(),
        d.size,
        d.length,
        d.plumb_tenths_of_degree / 10,
        d.plumb_tenths_of_degree % 10
    );
    let _ = writeln!(
        out,
        "  Birdsmouths: {:#} seat, {:#} heel; heels {:#} and {:#} along the bottom edge from the upper end",
        d.seat, d.heel, d.front_heel_at, d.back_heel_at
    );
    let _ = writeln!(out);
    members(&mut out, &r.members);
    marks(&mut out, &r.marks, from, "Rafter marks on the cap plates");
    splices(&mut out, &r.splices);
    notes(&mut out, &r.warnings);
    out
}

pub fn cut_list(cl: &CutList) -> String {
    let mut out = String::new();
    let s = cl.summary();
    let _ = writeln!(out, "Shopping list");
    let _ = writeln!(out);
    for line in cl.bom() {
        let price = match (line.unit_price_cents, line.total_cents) {
            (Some(each), Some(total)) => {
                format!("  @ {} = {}", dollars(each.into()), dollars(total))
            }
            _ => String::new(),
        };
        let _ = writeln!(
            out,
            "  {:>3} x {:<5} {:>4}{price}",
            line.count,
            line.size.to_string(),
            line.length.to_string()
        );
    }
    for line in cl.sheet_bom() {
        let price = match (line.unit_price_cents, line.total_cents) {
            (Some(each), Some(total)) => {
                format!("  @ {} = {}", dollars(each.into()), dollars(total))
            }
            _ => String::new(),
        };
        let _ = writeln!(
            out,
            "  {:>3} x {:#} sheet {} x {}{price}",
            line.count, line.thickness, line.width, line.length
        );
    }
    if let Some(total) = s.total_cents {
        let _ = writeln!(out, "  total {}", dollars(total));
    }
    let _ = writeln!(
        out,
        "  {} boards for {} pieces, {:.1}% waste (kerf {:#}, end trim {:#})",
        s.boards, s.pieces, s.waste_percent, cl.kerf, cl.end_trim
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "Cut list");
    for b in &cl.boards {
        let _ = writeln!(out);
        let _ = writeln!(out, "  {}  {} {}", b.id, b.size, b.length);
        for (i, p) in b.pieces.iter().enumerate() {
            let _ = writeln!(
                out,
                "    {}. {:>10}  {} ({})",
                i + 1,
                p.length.to_tape_string(),
                p.label,
                p.kind.name()
            );
        }
        let _ = writeln!(out, "       {:>10}  offcut", format!("{:#}", b.offcut));
    }
    for sheet in &cl.sheets {
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "  {}  {} x {} sheet",
            sheet.id, sheet.width, sheet.length
        );
        for (i, c) in sheet.pieces.iter().enumerate() {
            let _ = writeln!(
                out,
                "    {}. {:>16}  {}",
                i + 1,
                format!("{:#} x {:#}", c.piece.length, c.piece.width),
                c.piece.label
            );
        }
    }
    if !cl.too_long.is_empty() {
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "  Longer than any stock board, so not in the cut list:"
        );
        for p in &cl.too_long {
            let _ = writeln!(
                out,
                "    {:>10}  {} ({} {})",
                p.length.to_tape_string(),
                p.label,
                p.size,
                p.kind.name()
            );
        }
    }
    out
}

fn dollars(cents: u64) -> String {
    format!("${}.{:02}", cents / 100, cents % 100)
}
