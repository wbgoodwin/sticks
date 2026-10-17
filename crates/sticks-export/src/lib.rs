//! CSV, SVG elevation and printable-page exports.
//!
//! CSV is in place; elevations and printable pages come in milestone M3.

use sticks_core::{Cut, Length};
use sticks_cutlist::CutList;

/// The shopping list: one row per size and stock length.
pub fn bom_csv(cut_list: &CutList) -> String {
    let mut out = String::new();
    row(
        &mut out,
        ["size", "length", "feet", "quantity", "unit price", "total"],
    );
    for line in cut_list.bom() {
        row(
            &mut out,
            [
                line.size.to_string(),
                line.length.to_string(),
                feet(line.length),
                line.count.to_string(),
                dollars(line.unit_price_cents.map(u64::from)),
                dollars(line.total_cents),
            ],
        );
    }
    let total = cut_list.summary().total_cents;
    if total.is_some() {
        row(&mut out, ["total", "", "", "", "", &dollars(total)]);
    }
    out
}

/// The cut list: one row per piece, grouped by board in cutting order.
pub fn cut_list_csv(cut_list: &CutList) -> String {
    let mut out = String::new();
    row(
        &mut out,
        [
            "board", "size", "stock", "order", "label", "member", "length", "inches", "cuts",
            "offcut",
        ],
    );
    for b in &cut_list.boards {
        for (i, p) in b.pieces.iter().enumerate() {
            let last = i + 1 == b.pieces.len();
            row(
                &mut out,
                [
                    b.id.clone(),
                    b.size.to_string(),
                    b.length.to_string(),
                    (i + 1).to_string(),
                    p.label.clone(),
                    p.kind.name().to_string(),
                    p.length.to_tape_string(),
                    inches(p.length),
                    cuts(&p.cuts),
                    if last {
                        format!("{:#}", b.offcut)
                    } else {
                        String::new()
                    },
                ],
            );
        }
    }
    for p in &cut_list.too_long {
        row(
            &mut out,
            [
                "too long for stock".to_string(),
                p.size.to_string(),
                String::new(),
                String::new(),
                p.label.clone(),
                p.kind.name().to_string(),
                p.length.to_tape_string(),
                inches(p.length),
                cuts(&p.cuts),
                String::new(),
            ],
        );
    }
    out
}

fn row<S: AsRef<str>>(out: &mut String, fields: impl IntoIterator<Item = S>) {
    for (i, field) in fields.into_iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let f = field.as_ref();
        if f.contains([',', '"', '\n', '\r']) {
            out.push('"');
            out.push_str(&f.replace('"', "\"\""));
            out.push('"');
        } else {
            out.push_str(f);
        }
    }
    out.push_str("\r\n");
}

/// Decimal inches for spreadsheets, e.g. `92.625`.
fn inches(l: Length) -> String {
    let s = format!("{:.4}", l.as_inches_f64());
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn feet(l: Length) -> String {
    let s = format!("{:.4}", l.as_inches_f64() / 12.0);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn dollars(cents: Option<u64>) -> String {
    cents.map_or(String::new(), |c| format!("{}.{:02}", c / 100, c % 100))
}

fn cuts(cuts: &[Cut]) -> String {
    cuts.iter()
        .map(|c| match c {
            Cut::Square => "square".to_string(),
            Cut::Angled { tenths_of_degree } => {
                format!(
                    "{}.{} deg",
                    tenths_of_degree / 10,
                    (tenths_of_degree % 10).abs()
                )
            }
            Cut::Birdsmouth => "birdsmouth".to_string(),
            Cut::Plumb => "plumb".to_string(),
        })
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use sticks_core::{Member, MemberKind, Nominal, Rules, Stock};
    use sticks_cutlist::cut_list;

    fn sample() -> CutList {
        let stud = |label: &str| Member {
            label: label.into(),
            kind: MemberKind::Stud,
            size: Nominal::new(2, 4),
            length: Length::from_sixteenths(92 * 16 + 10),
            cuts: Vec::new(),
        };
        let stock = [Stock {
            size: Nominal::new(2, 4),
            lengths: vec![Length::feet(8), Length::feet(16)],
            prices_cents: vec![498, 1149],
        }];
        let mut long = stud("TP1");
        long.length = Length::feet(18);
        long.kind = MemberKind::TopPlate;
        cut_list(&[stud("S01"), stud("S02"), long], &stock, &Rules::default()).unwrap()
    }

    #[test]
    fn bom() {
        assert_eq!(
            bom_csv(&sample()),
            "size,length,feet,quantity,unit price,total\r\n\
             2x4,8',8,2,4.98,9.96\r\n\
             total,,,,,9.96\r\n"
        );
    }

    #[test]
    fn cut_list_quotes_inch_marks() {
        let csv = cut_list_csv(&sample());
        let lines: Vec<_> = csv.lines().collect();
        assert_eq!(
            lines[1],
            "2x4-01,2x4,8',1,S01,stud,\"92-5/8\"\"\",92.625,,\"3-1/4\"\"\""
        );
        assert_eq!(lines[3], "too long for stock,2x4,,,TP1,top plate,18',216,,");
    }
}
