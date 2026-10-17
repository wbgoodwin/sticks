//! Plain-text output for a terminal or a printout.

use std::fmt::Write;

use sticks_core::{LayoutStart, MemberKind};
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
            kind_name(m.kind),
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
            kind_name(m.kind)
        );
    }
    for warning in &w.warnings {
        let _ = writeln!(out, "  note: {warning}");
    }
    let _ = writeln!(out);
    out
}

pub fn kind_name(kind: MemberKind) -> &'static str {
    match kind {
        MemberKind::BottomPlate => "bottom plate",
        MemberKind::TopPlate => "top plate",
        MemberKind::CapPlate => "cap plate",
        MemberKind::Stud => "stud",
        MemberKind::KingStud => "king stud",
        MemberKind::JackStud => "jack stud",
        MemberKind::Cripple => "cripple",
        MemberKind::Header => "header",
        MemberKind::Sill => "sill",
        MemberKind::Blocking => "blocking",
        MemberKind::RimJoist => "rim joist",
        MemberKind::Joist => "joist",
        MemberKind::Beam => "beam",
        MemberKind::Ledger => "ledger",
        MemberKind::Rafter => "rafter",
        MemberKind::Fascia => "fascia",
        MemberKind::Post => "post",
    }
}
