//! Cutting sheet goods (subfloor, later sheathing) from whole sheets.
//!
//! Pieces are packed onto sheets in shelves: tallest pieces first, each shelf
//! as tall as its first piece and filled along the sheet's length. Pieces in
//! a frame layout are mostly whole sheets or long strips, which shelves suit.
//! No kerf is taken between sheet pieces: edge pieces run up to a blade's
//! width short, which a subfloor hides.

use serde::Serialize;
use sticks_core::{Length, SheetPiece, SheetStock};

use crate::CutListError;

/// One whole sheet and the pieces cut from it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Sheet {
    /// Sheet label for the saw, e.g. `3/4"-02`.
    pub id: String,
    pub thickness: Length,
    pub length: Length,
    pub width: Length,
    pub price_cents: Option<u32>,
    pub pieces: Vec<SheetCut>,
}

/// A piece and where it comes from on its sheet: `x` along the length, `y`
/// across the width, from one corner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SheetCut {
    pub piece: SheetPiece,
    pub x: Length,
    pub y: Length,
}

/// A line on the shopping list: sheets of one thickness and size.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SheetLine {
    pub thickness: Length,
    pub length: Length,
    pub width: Length,
    pub count: usize,
    pub unit_price_cents: Option<u32>,
    pub total_cents: Option<u64>,
}

struct Shelf {
    y: Length,
    height: Length,
    used: Length,
}

struct Open {
    sheet: Sheet,
    shelves: Vec<Shelf>,
}

/// Plans whole sheets for `pieces`, using the stock of each thickness or a
/// plain unpriced 4x8 when none is listed.
pub fn plan_sheets(
    pieces: &[SheetPiece],
    stock: &[SheetStock],
) -> Result<Vec<Sheet>, CutListError> {
    let mut thicknesses: Vec<Length> = pieces.iter().map(|p| p.thickness).collect();
    thicknesses.sort();
    thicknesses.dedup();
    let mut out = Vec::new();
    for thickness in thicknesses {
        let stock = stock
            .iter()
            .find(|s| s.thickness == thickness)
            .cloned()
            .unwrap_or_else(|| SheetStock::standard(thickness));
        let mut todo: Vec<&SheetPiece> =
            pieces.iter().filter(|p| p.thickness == thickness).collect();
        if let Some(p) = todo
            .iter()
            .find(|p| p.length > stock.length || p.width > stock.width)
        {
            return Err(CutListError::SheetTooSmall {
                label: p.label.clone(),
                length: stock.length,
                width: stock.width,
            });
        }
        todo.sort_by(|a, b| {
            b.width
                .cmp(&a.width)
                .then_with(|| b.length.cmp(&a.length))
                .then_with(|| a.label.cmp(&b.label))
        });
        let mut open: Vec<Open> = Vec::new();
        for piece in todo {
            let spot = open.iter().enumerate().find_map(|(i, o)| {
                o.shelves
                    .iter()
                    .position(|s| piece.width <= s.height && s.used + piece.length <= stock.length)
                    .map(|s| (i, s))
            });
            let (i, s) = match spot {
                Some(spot) => spot,
                None => {
                    let room = open.iter().position(|o| {
                        let top = o
                            .shelves
                            .iter()
                            .map(|s| s.y + s.height)
                            .max()
                            .unwrap_or_default();
                        top + piece.width <= stock.width
                    });
                    let i = room.unwrap_or_else(|| {
                        open.push(Open {
                            sheet: Sheet {
                                id: String::new(),
                                thickness,
                                length: stock.length,
                                width: stock.width,
                                price_cents: stock.price_cents,
                                pieces: Vec::new(),
                            },
                            shelves: Vec::new(),
                        });
                        open.len() - 1
                    });
                    let o = &mut open[i];
                    let y = o
                        .shelves
                        .iter()
                        .map(|s| s.y + s.height)
                        .max()
                        .unwrap_or_default();
                    o.shelves.push(Shelf {
                        y,
                        height: piece.width,
                        used: Length::ZERO,
                    });
                    (i, o.shelves.len() - 1)
                }
            };
            let o = &mut open[i];
            let shelf = &mut o.shelves[s];
            o.sheet.pieces.push(SheetCut {
                piece: piece.clone(),
                x: shelf.used,
                y: shelf.y,
            });
            shelf.used += piece.length;
        }
        for (n, o) in open.into_iter().enumerate() {
            let mut sheet = o.sheet;
            sheet.id = format!("{thickness:#}-{:02}", n + 1);
            out.push(sheet);
        }
    }
    Ok(out)
}

/// Sheets grouped by thickness and size for the shopping list.
pub fn sheet_bom(sheets: &[Sheet]) -> Vec<SheetLine> {
    let mut lines: Vec<SheetLine> = Vec::new();
    for s in sheets {
        match lines
            .iter_mut()
            .find(|l| l.thickness == s.thickness && l.length == s.length && l.width == s.width)
        {
            Some(line) => line.count += 1,
            None => lines.push(SheetLine {
                thickness: s.thickness,
                length: s.length,
                width: s.width,
                count: 1,
                unit_price_cents: s.price_cents,
                total_cents: None,
            }),
        }
    }
    for line in &mut lines {
        line.total_cents = line.unit_price_cents.map(|p| p as u64 * line.count as u64);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piece(n: usize, length: i64, width: i64) -> SheetPiece {
        SheetPiece {
            label: format!("SF{n:02}"),
            thickness: Length::from_sixteenths(12),
            length: Length::inches(length),
            width: Length::inches(width),
        }
    }

    #[test]
    fn whole_sheets_and_strips_share_sheets() {
        let pieces = [
            piece(1, 96, 48),
            piece(2, 72, 48),
            piece(3, 24, 48),
            piece(4, 96, 24),
            piece(5, 96, 24),
        ];
        let sheets = plan_sheets(&pieces, &[]).unwrap();
        assert_eq!(sheets.len(), 3);
        assert_eq!(sheets[0].id, "3/4\"-01");
        // Two 2' strips stack on one sheet.
        assert_eq!(sheets[2].pieces.len(), 2);
        assert_eq!(sheets[2].pieces[1].y, Length::inches(24));
        let lines = sheet_bom(&sheets);
        assert_eq!(lines[0].count, 3);
        assert_eq!(lines[0].total_cents, None);
    }

    #[test]
    fn pieces_bigger_than_a_sheet_are_errors() {
        let err = plan_sheets(&[piece(1, 120, 48)], &[]).unwrap_err();
        assert!(err.to_string().contains("SF01"), "{err}");
    }
}
