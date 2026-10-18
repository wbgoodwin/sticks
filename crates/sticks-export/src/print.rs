//! The build packet: one HTML page to print, with the shopping list, every
//! drawing on its own sheet, and the board-by-board cut list.

use std::fmt::Write;

use sticks_core::Project;
use sticks_cutlist::CutList;
use sticks_rules::FramedProject;

use crate::svg::{
    deck_svg, floor_svg, porch_svg, rafter_svg, roof_svg, sheathing_svg, subfloor_svg, wall_svg,
    NOTICE,
};

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn dollars(cents: u64) -> String {
    format!("${}.{:02}", cents / 100, cents % 100)
}

pub fn print_html(project: &Project, framed: &FramedProject, cl: &CutList) -> String {
    let from = project.rules.layout_from;
    let name = esc(&project.name);
    let mut out = String::new();
    let _ = write!(
        out,
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>{name}: build packet</title>
<style>
@page {{ size: letter landscape; margin: 0; }}
* {{ box-sizing: border-box; }}
body {{ margin: 0; font: 10pt/1.35 Helvetica, Arial, sans-serif; color: #111; background: #ddd; }}
.page {{ width: 11in; height: 8.5in; margin: 0.25in auto; background: #fff; overflow: hidden; break-after: page; }}
.page > svg {{ display: block; width: 11in; height: 8.5in; }}
.text {{ padding: 0.5in; height: auto; min-height: 8.5in; }}
h1 {{ font-size: 18pt; margin: 0 0 4pt; }}
h2 {{ font-size: 12pt; margin: 14pt 0 4pt; }}
table {{ border-collapse: collapse; margin: 4pt 0; }}
th, td {{ border-bottom: 0.5pt solid #bbb; padding: 2pt 8pt 2pt 0; text-align: left; vertical-align: top; }}
td.num, th.num {{ text-align: right; }}
.boards {{ columns: 2; column-gap: 0.4in; }}
.board {{ break-inside: avoid; margin-bottom: 8pt; }}
.board h3 {{ font-size: 10pt; margin: 0 0 2pt; }}
.notice {{ color: #666; font-size: 8pt; margin-top: 12pt; }}
.problem {{ color: #a33; }}
@media print {{ body {{ background: none; }} .page {{ margin: 0; }} }}
</style>
</head>
<body>
<section class="page text">
<h1>{name}</h1>
"#
    );

    let s = cl.summary();
    let cost = s
        .total_cents
        .map(|c| format!(", {}", dollars(c)))
        .unwrap_or_default();
    let _ = writeln!(
        out,
        "<p>{} boards and {} sheets for {} pieces, {:.1}% lumber waste{cost}. Kerf {:#}, end trim {:#}.</p>",
        s.boards, s.sheets, s.pieces, s.waste_percent, cl.kerf, cl.end_trim
    );
    let on_hand = framed.on_hand_names(&project.on_hand);
    if !on_hand.is_empty() {
        let _ = writeln!(
            out,
            "<p>Not on the shopping list, since you have it: {}.</p>",
            esc(&on_hand.join(", "))
        );
    }
    for e in framed.errors() {
        let _ = writeln!(
            out,
            "<p class=\"problem\">Left out: {}</p>",
            esc(&e.to_string())
        );
    }
    out.push_str("<h2>Shopping list</h2>\n<table>\n<tr><th class=\"num\">Qty</th><th>Size</th><th>Length</th><th class=\"num\">Each</th><th class=\"num\">Total</th></tr>\n");
    let price = |p: Option<u64>| p.map(dollars).unwrap_or_default();
    for l in cl.bom() {
        let _ = writeln!(
            out,
            "<tr><td class=\"num\">{}</td><td>{}</td><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td></tr>",
            l.count,
            l.size,
            esc(&l.length.to_string()),
            price(l.unit_price_cents.map(u64::from)),
            price(l.total_cents)
        );
    }
    for l in cl.sheet_bom() {
        let _ = writeln!(
            out,
            "<tr><td class=\"num\">{}</td><td>{} sheet</td><td>{} x {}</td><td class=\"num\">{}</td><td class=\"num\">{}</td></tr>",
            l.count,
            esc(&format!("{:#}", l.thickness)),
            esc(&l.width.to_string()),
            esc(&l.length.to_string()),
            price(l.unit_price_cents.map(u64::from)),
            price(l.total_cents)
        );
    }
    if let Some(total) = s.total_cents {
        let _ = writeln!(
            out,
            "<tr><td></td><td><strong>Total</strong></td><td></td><td></td><td class=\"num\"><strong>{}</strong></td></tr>",
            dollars(total)
        );
    }
    out.push_str("</table>\n");
    if !cl.too_long.is_empty() {
        out.push_str("<h2 class=\"problem\">Longer than any stock board</h2>\n<table>\n");
        for p in &cl.too_long {
            let _ = writeln!(
                out,
                "<tr><td>{}</td><td>{} {}</td><td>{}</td></tr>",
                esc(&p.label),
                p.size,
                p.kind.name(),
                esc(&p.length.to_tape_string())
            );
        }
        out.push_str("</table>\n");
    }
    let mut piers = Vec::new();
    if let Some(Ok(floor)) = &framed.floor {
        piers.push(format!("{} under the floor", floor.piers.len()));
    }
    if let Some(Ok(porch)) = &framed.porch {
        match &porch.deck {
            Some(d) => piers.push(format!("{} under the porch deck", d.piers.len())),
            None => piers.push(format!("{} under the porch posts", porch.piers.len())),
        }
    }
    if !piers.is_empty() {
        let _ = writeln!(
            out,
            "<h2>Piers</h2>\n<p>{}; positions on the framing plans.</p>",
            piers.join(", ")
        );
    }
    let mut warnings: Vec<String> = Vec::new();
    for w in framed.walls.iter().flatten() {
        warnings.extend(w.warnings.iter().map(|x| format!("Wall {}: {x}", w.id)));
    }
    if let Some(Ok(f)) = &framed.floor {
        warnings.extend(f.warnings.iter().map(|x| format!("Floor: {x}")));
    }
    if let Some(Ok(r)) = &framed.roof {
        warnings.extend(r.warnings.iter().map(|x| format!("Roof: {x}")));
    }
    if let Some(Ok(p)) = &framed.porch {
        warnings.extend(p.warnings.iter().map(|x| format!("Porch: {x}")));
        if let Some(d) = &p.deck {
            warnings.extend(d.warnings.iter().map(|x| format!("Porch deck: {x}")));
        }
    }
    if let Some(Ok(s)) = &framed.sheathing {
        warnings.extend(s.warnings.iter().map(|x| format!("Sheathing: {x}")));
    }
    if !warnings.is_empty() {
        out.push_str("<h2>Worth a second look</h2>\n<ul>\n");
        for w in warnings {
            let _ = writeln!(out, "<li>{}</li>", esc(&w));
        }
        out.push_str("</ul>\n");
    }
    let _ = writeln!(out, "<p class=\"notice\">{}</p>\n</section>", esc(NOTICE));

    let mut page = |svg: String| {
        let _ = writeln!(out, "<section class=\"page\">\n{svg}</section>");
    };
    for w in framed.walls.iter().flatten() {
        page(wall_svg(&project.name, w, from));
    }
    if let Some(Ok(floor)) = &framed.floor {
        page(floor_svg(&project.name, floor, from));
        page(subfloor_svg(&project.name, floor));
    }
    if let Some(Ok(roof)) = &framed.roof {
        page(roof_svg(&project.name, roof, from));
        for r in &roof.rafters {
            page(rafter_svg(&project.name, r));
        }
    }
    if let Some(Ok(porch)) = &framed.porch {
        page(porch_svg(&project.name, porch, from));
        page(rafter_svg(&project.name, &porch.rafter));
        if let Some(svg) = deck_svg(&project.name, porch, from) {
            page(svg);
        }
    }
    if let Some(Ok(sheathing)) = &framed.sheathing {
        for roofs in [false, true] {
            if let Some(svg) = sheathing_svg(&project.name, sheathing, roofs) {
                page(svg);
            }
        }
    }

    out.push_str("<section class=\"page text\">\n<h1>Cut list</h1>\n<div class=\"boards\">\n");
    for b in &cl.boards {
        let _ = writeln!(
            out,
            "<div class=\"board\"><h3>{} &middot; {} {}</h3><table>",
            esc(&b.id),
            b.size,
            esc(&b.length.to_string())
        );
        for (i, p) in b.pieces.iter().enumerate() {
            let cuts: Vec<String> = p
                .cuts
                .iter()
                .filter(|c| !matches!(c, sticks_core::Cut::Square))
                .map(|c| c.describe())
                .collect();
            let _ = writeln!(
                out,
                "<tr><td>{}.</td><td class=\"num\">{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                i + 1,
                esc(&p.length.to_tape_string()),
                esc(&p.label),
                p.kind.name(),
                esc(&cuts.join("; "))
            );
        }
        let _ = writeln!(
            out,
            "<tr><td></td><td class=\"num\">{}</td><td colspan=\"3\">offcut</td></tr></table></div>",
            esc(&format!("{:#}", b.offcut))
        );
    }
    for sheet in &cl.sheets {
        let _ = writeln!(
            out,
            "<div class=\"board\"><h3>{} &middot; {} x {} sheet</h3><table>",
            esc(&sheet.id),
            esc(&sheet.width.to_string()),
            esc(&sheet.length.to_string())
        );
        for (i, c) in sheet.pieces.iter().enumerate() {
            let _ = writeln!(
                out,
                "<tr><td>{}.</td><td class=\"num\">{}</td><td>{}</td><td>{}</td></tr>",
                i + 1,
                esc(&format!("{:#} x {:#}", c.piece.length, c.piece.width)),
                esc(&c.piece.label),
                c.piece.used_for.name()
            );
        }
        out.push_str("</table></div>\n");
    }
    out.push_str("</div>\n</section>\n</body>\n</html>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use sticks_cutlist::cut_list_with_sheets;
    use sticks_rules::frame_project;

    #[test]
    fn packet_has_a_page_per_drawing() {
        let p = Project::from_toml(include_str!("../../../examples/shed.toml")).unwrap();
        let framed = frame_project(&p).unwrap();
        let cl = cut_list_with_sheets(
            &framed.members(),
            &framed.sheet_pieces(),
            &p.stock,
            &p.sheets,
            &p.rules,
        )
        .unwrap();
        let html = print_html(&p, &framed, &cl);
        // Cover, four walls, floor, subfloor, roof, rafter, porch, porch
        // rafter, deck, wall and roof sheathing, cut list.
        assert_eq!(html.matches("<section class=\"page").count(), 15);
        assert!(html.contains("wall sheathing</td>"));
        assert!(html.contains("<h2>Shopping list</h2>"));
        assert!(html.contains("3/4&quot; sheet"));
        assert!(html.contains("birdsmouth"));
        assert!(!html.contains("Longer than any stock board"));
    }
}
