//! Plain-text output for a terminal or a printout.

use std::fmt::Write;

use sticks_core::LayoutStart;
use sticks_cutlist::CutList;
use sticks_rules::FramedWall;

pub fn wall(w: &FramedWall, from: LayoutStart) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "Wall {}: {} long, {} tall", w.id, w.length, w.height);
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "  {:<10} {:<13} {:<5} {:>10}",
        "label", "member", "size", "length"
    );
    for p in &w.members {
        let m = &p.member;
        let _ = writeln!(
            out,
            "  {:<10} {:<13} {:<5} {:>10}",
            m.label,
            m.kind.name(),
            m.size.to_string(),
            m.length.to_tape_string()
        );
    }
    let _ = writeln!(out);
    let end = match from {
        LayoutStart::Left => "left",
        LayoutStart::Right => "right",
    };
    let _ = writeln!(out, "  Layout marks, measured from the {end} end:");
    for m in &w.marks {
        let _ = writeln!(
            out,
            "  {:>10}  {} ({})",
            format!("{:#}", m.at),
            m.label,
            m.kind.name()
        );
    }
    for warning in &w.warnings {
        let _ = writeln!(out, "  note: {warning}");
    }
    let _ = writeln!(out);
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
