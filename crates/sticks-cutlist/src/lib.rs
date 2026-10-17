//! Bill of materials and the cut-list optimizer.
//!
//! The optimizer solves a one-dimensional cutting-stock problem for each
//! nominal size: cut every member from the cheapest set of stock boards,
//! losing a saw kerf between pieces and an end-trim allowance per board.
//!
//! v1 is best-fit decreasing. Pieces go longest first onto the open board
//! they leave the least room on; when none fits, a new board is opened. The
//! choice of which stock length to open is run several ways (cheapest that
//! fits, and each stock length in turn) and the cheapest plan wins. Every
//! board is then cut down to the cheapest stock length that still holds it.
//!
//! Pieces longer than the longest stock are never split here; they come back
//! in [`CutList::too_long`] so the frame can be changed instead. The framing
//! rules already split plates, rims, beams and fascia over their supports.
//!
//! Sheet goods (the subfloor) are planned separately in [`sheets`] and ride
//! along on the same [`CutList`] so the shopping list and total cover both.

pub mod sheets;

use std::collections::BTreeMap;

use serde::Serialize;
use sticks_core::{Length, Member, Nominal, Rules, SheetPiece, SheetStock, Stock};

pub use sheets::{plan_sheets, sheet_bom, Sheet, SheetCut, SheetLine};
/// Stock lengths assumed for a size the project lists no stock for.
pub use sticks_core::DEFAULT_STOCK_FEET;

/// One stock board and the pieces cut from it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Board {
    /// Board label for the saw, e.g. `2x4-03`.
    pub id: String,
    pub size: Nominal,
    pub length: Length,
    pub price_cents: Option<u32>,
    /// Pieces in cutting order, measured off from one end.
    pub pieces: Vec<Member>,
    /// What's left after the last cut and its kerf.
    pub offcut: Length,
}

impl Board {
    /// Length the pieces use up, counting end trim and the kerf between pieces.
    pub fn used(&self, kerf: Length, end_trim: Length) -> Length {
        needed(self.pieces.iter().map(|p| p.length), kerf, end_trim)
    }
}

/// A line on the shopping list: boards of one size and length.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BomLine {
    pub size: Nominal,
    pub length: Length,
    pub count: usize,
    pub unit_price_cents: Option<u32>,
    pub total_cents: Option<u64>,
}

/// Totals across the whole cut list.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Summary {
    pub boards: usize,
    pub sheets: usize,
    /// Pieces cut, which is also what a one-piece-per-board takeoff buys.
    pub pieces: usize,
    /// Total cost, when every board bought has a price.
    pub total_cents: Option<u64>,
    /// Lumber bought, end to end.
    pub bought: Length,
    /// Lumber that ends up in the frame.
    pub used: Length,
    /// Kerf, trim and offcuts as a percent of lumber bought.
    pub waste_percent: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CutList {
    pub kerf: Length,
    pub end_trim: Length,
    /// Ordered by size, then stock length (longest first).
    pub boards: Vec<Board>,
    /// Pieces longer than the longest stock board of their size.
    pub too_long: Vec<Member>,
    /// Whole sheets of subfloor and the pieces cut from them.
    pub sheets: Vec<Sheet>,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CutListError {
    #[error("{size} stock lists {lengths} lengths but {prices} prices; give a price for every length or none")]
    PriceCount {
        size: Nominal,
        lengths: usize,
        prices: usize,
    },
    #[error("{size} stock needs at least one length")]
    NoLengths { size: Nominal },
    #[error("{size} stock length {length} is too short to cut anything from")]
    BadLength { size: Nominal, length: Length },
    #[error("{size} is listed as stock more than once")]
    Duplicate { size: Nominal },
    #[error("kerf and end trim can't be negative")]
    Negative,
    #[error("{label} is bigger than a {length} x {width} sheet")]
    SheetTooSmall {
        label: String,
        length: Length,
        width: Length,
    },
}

/// A stock length on offer and what it costs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Offer {
    length: Length,
    price_cents: Option<u32>,
}

impl Offer {
    /// What the optimizer minimizes: the price when known, otherwise the
    /// length, so unpriced plans buy the least lumber.
    fn cost(self) -> i64 {
        match self.price_cents {
            Some(c) => c as i64,
            None => self.length.sixteenths(),
        }
    }
}

/// Plans boards for `members`, using the project's stock and its kerf and
/// end trim. Sizes with no stock listed get [`DEFAULT_STOCK_FEET`], unpriced.
pub fn cut_list(
    members: &[Member],
    stock: &[Stock],
    rules: &Rules,
) -> Result<CutList, CutListError> {
    if rules.kerf < Length::ZERO || rules.end_trim < Length::ZERO {
        return Err(CutListError::Negative);
    }
    let offers = offers(stock, rules.end_trim)?;

    let mut by_size: BTreeMap<Nominal, Vec<Member>> = BTreeMap::new();
    for m in members {
        by_size.entry(m.size).or_default().push(m.clone());
    }

    let mut boards = Vec::new();
    let mut too_long = Vec::new();
    for (size, mut pieces) in by_size {
        let menu = offers.get(&size).cloned().unwrap_or_else(default_offers);
        // Longest first; labels break ties so the plan is stable.
        pieces.sort_by(|a, b| b.length.cmp(&a.length).then_with(|| a.label.cmp(&b.label)));
        let longest = menu.iter().map(|o| o.length).max().unwrap_or(Length::ZERO);
        let (fits, long): (Vec<_>, Vec<_>) = pieces
            .into_iter()
            .partition(|p| needed([p.length], rules.kerf, rules.end_trim) <= longest);
        too_long.extend(long);

        let mut plan = best_plan(&fits, &menu, rules.kerf, rules.end_trim);
        plan.sort_by(|a, b| {
            b.offer
                .length
                .cmp(&a.offer.length)
                .then_with(|| b.pieces.len().cmp(&a.pieces.len()))
        });
        for (i, open) in plan.into_iter().enumerate() {
            let used = needed(
                open.pieces.iter().map(|p| p.length),
                rules.kerf,
                rules.end_trim,
            );
            boards.push(Board {
                id: format!("{size}-{:02}", i + 1),
                size,
                length: open.offer.length,
                price_cents: open.offer.price_cents,
                pieces: open.pieces,
                offcut: (open.offer.length - used - rules.kerf).max(Length::ZERO),
            });
        }
    }
    Ok(CutList {
        kerf: rules.kerf,
        end_trim: rules.end_trim,
        boards,
        too_long,
        sheets: Vec::new(),
    })
}

/// [`cut_list`] for the lumber, plus whole sheets for `sheet_pieces`.
pub fn cut_list_with_sheets(
    members: &[Member],
    sheet_pieces: &[SheetPiece],
    stock: &[Stock],
    sheet_stock: &[SheetStock],
    rules: &Rules,
) -> Result<CutList, CutListError> {
    let mut cl = cut_list(members, stock, rules)?;
    cl.sheets = plan_sheets(sheet_pieces, sheet_stock)?;
    Ok(cl)
}

impl CutList {
    /// Sheet goods on the shopping list.
    pub fn sheet_bom(&self) -> Vec<SheetLine> {
        sheet_bom(&self.sheets)
    }

    /// The shopping list: boards grouped by size and stock length.
    pub fn bom(&self) -> Vec<BomLine> {
        let mut lines: Vec<BomLine> = Vec::new();
        for b in &self.boards {
            match lines
                .iter_mut()
                .find(|l| l.size == b.size && l.length == b.length)
            {
                Some(line) => line.count += 1,
                None => lines.push(BomLine {
                    size: b.size,
                    length: b.length,
                    count: 1,
                    unit_price_cents: b.price_cents,
                    total_cents: None,
                }),
            }
        }
        for line in &mut lines {
            line.total_cents = line.unit_price_cents.map(|p| p as u64 * line.count as u64);
        }
        lines.sort_by(|a, b| a.size.cmp(&b.size).then_with(|| b.length.cmp(&a.length)));
        lines
    }

    pub fn summary(&self) -> Summary {
        let bought: Length = self.boards.iter().map(|b| b.length).sum();
        let used: Length = self
            .boards
            .iter()
            .flat_map(|b| &b.pieces)
            .map(|p| p.length)
            .sum();
        let pieces = self.boards.iter().map(|b| b.pieces.len()).sum();
        let total_cents = self
            .boards
            .iter()
            .map(|b| b.price_cents)
            .chain(self.sheets.iter().map(|s| s.price_cents))
            .map(|p| p.map(u64::from))
            .sum::<Option<u64>>();
        let waste_percent = if bought > Length::ZERO {
            (bought - used).sixteenths() as f64 * 100.0 / bought.sixteenths() as f64
        } else {
            0.0
        };
        Summary {
            boards: self.boards.len(),
            sheets: self.sheets.len(),
            pieces,
            total_cents: if self.boards.is_empty() && self.sheets.is_empty() {
                None
            } else {
                total_cents
            },
            bought,
            used,
            waste_percent,
        }
    }
}

fn default_offers() -> Vec<Offer> {
    DEFAULT_STOCK_FEET
        .iter()
        .map(|&f| Offer {
            length: Length::feet(f),
            price_cents: None,
        })
        .collect()
}

fn offers(
    stock: &[Stock],
    end_trim: Length,
) -> Result<BTreeMap<Nominal, Vec<Offer>>, CutListError> {
    let mut out = BTreeMap::new();
    for s in stock {
        if s.lengths.is_empty() {
            return Err(CutListError::NoLengths { size: s.size });
        }
        if !s.prices_cents.is_empty() && s.prices_cents.len() != s.lengths.len() {
            return Err(CutListError::PriceCount {
                size: s.size,
                lengths: s.lengths.len(),
                prices: s.prices_cents.len(),
            });
        }
        if let Some(&length) = s.lengths.iter().find(|&&l| l <= end_trim) {
            return Err(CutListError::BadLength {
                size: s.size,
                length,
            });
        }
        let menu = s
            .lengths
            .iter()
            .enumerate()
            .map(|(i, &length)| Offer {
                length,
                price_cents: s.prices_cents.get(i).copied(),
            })
            .collect();
        if out.insert(s.size, menu).is_some() {
            return Err(CutListError::Duplicate { size: s.size });
        }
    }
    Ok(out)
}

/// Length a board must have to yield `pieces`: end trim, the pieces, and a
/// kerf between each pair. The last piece may run to the board's end.
fn needed(pieces: impl IntoIterator<Item = Length>, kerf: Length, end_trim: Length) -> Length {
    let mut total = end_trim;
    let mut count = 0;
    for p in pieces {
        total += p;
        count += 1;
    }
    if count > 1 {
        total += kerf * (count - 1);
    }
    total
}

#[derive(Clone, Debug)]
struct Open {
    offer: Offer,
    pieces: Vec<Member>,
    /// `needed` for the pieces so far.
    used: Length,
}

impl Open {
    /// Room left if `piece` were added, or `None` if it doesn't fit.
    fn room_after(&self, piece: Length, kerf: Length) -> Option<Length> {
        let gap = if self.pieces.is_empty() {
            Length::ZERO
        } else {
            kerf
        };
        let room = self.offer.length - self.used - gap - piece;
        (room >= Length::ZERO).then_some(room)
    }

    fn add(&mut self, piece: Member, kerf: Length) {
        if !self.pieces.is_empty() {
            self.used += kerf;
        }
        self.used += piece.length;
        self.pieces.push(piece);
    }
}

/// How to pick a stock length when no open board fits the next piece.
#[derive(Clone, Copy)]
enum Opening {
    /// The cheapest length that fits.
    Cheapest,
    /// This length when the piece fits it, else the cheapest that fits.
    Prefer(Length),
}

fn best_plan(pieces: &[Member], menu: &[Offer], kerf: Length, end_trim: Length) -> Vec<Open> {
    let mut policies = vec![Opening::Cheapest];
    policies.extend(menu.iter().map(|o| Opening::Prefer(o.length)));
    policies
        .into_iter()
        .map(|policy| {
            let mut plan = best_fit_decreasing(pieces, menu, kerf, end_trim, policy);
            downsize(&mut plan, menu);
            plan
        })
        .min_by_key(|plan| {
            let cost: i64 = plan.iter().map(|o| o.offer.cost()).sum();
            let bought: Length = plan.iter().map(|o| o.offer.length).sum();
            (cost, plan.len(), bought)
        })
        .unwrap_or_default()
}

fn best_fit_decreasing(
    pieces: &[Member],
    menu: &[Offer],
    kerf: Length,
    end_trim: Length,
    policy: Opening,
) -> Vec<Open> {
    let mut open: Vec<Open> = Vec::new();
    for piece in pieces {
        let best = open
            .iter()
            .enumerate()
            .filter_map(|(i, b)| b.room_after(piece.length, kerf).map(|room| (room, i)))
            .min();
        let i = match best {
            Some((_, i)) => i,
            None => {
                let fits = |o: &&Offer| o.length - end_trim >= piece.length;
                let cheapest = menu
                    .iter()
                    .filter(fits)
                    .min_by_key(|o| (o.cost(), o.length));
                let offer = match policy {
                    Opening::Prefer(len) => menu
                        .iter()
                        .filter(fits)
                        .find(|o| o.length == len)
                        .or(cheapest),
                    Opening::Cheapest => cheapest,
                };
                // Callers drop pieces that fit no stock before planning.
                let offer = *offer.expect("piece fits some stock");
                open.push(Open {
                    offer,
                    pieces: Vec::new(),
                    used: end_trim,
                });
                open.len() - 1
            }
        };
        open[i].add(piece.clone(), kerf);
    }
    open
}

/// Moves each board to the cheapest stock length that still holds its pieces.
fn downsize(plan: &mut [Open], menu: &[Offer]) {
    for board in plan {
        if let Some(cheaper) = menu
            .iter()
            .filter(|o| o.length >= board.used)
            .min_by_key(|o| (o.cost(), o.length))
        {
            if cheaper.cost() < board.offer.cost() {
                board.offer = *cheaper;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use sticks_core::{MemberKind, Project};

    fn piece(label: &str, inches_16: i64) -> Member {
        Member {
            label: label.into(),
            kind: MemberKind::Stud,
            size: Nominal::new(2, 4),
            length: Length::from_sixteenths(inches_16),
            cuts: Vec::new(),
        }
    }

    fn stock(feet: &[i64], prices: &[u32]) -> Vec<Stock> {
        vec![Stock {
            size: Nominal::new(2, 4),
            lengths: feet.iter().map(|&f| Length::feet(f)).collect(),
            prices_cents: prices.to_vec(),
        }]
    }

    #[test]
    fn packs_short_pieces_onto_one_board() {
        // Three 2' pieces and two kerfs fit an 8' board with room to spare.
        let pieces: Vec<_> = (0..3).map(|i| piece(&format!("P{i}"), 24 * 16)).collect();
        let cl = cut_list(&pieces, &stock(&[8], &[]), &Rules::default()).unwrap();
        assert_eq!(cl.boards.len(), 1);
        assert_eq!(cl.boards[0].id, "2x4-01");
        // 96 - 72 - 2 kerfs - the kerf after the last piece.
        assert_eq!(cl.boards[0].offcut, Length::from_sixteenths(24 * 16 - 6));

        let mut four = pieces.clone();
        four.push(piece("P3", 24 * 16));
        let cl = cut_list(&four, &stock(&[8], &[]), &Rules::default()).unwrap();
        assert_eq!(cl.boards.len(), 2, "kerf leaves no room for the fourth");
    }

    #[test]
    fn last_piece_may_use_the_whole_board() {
        let pieces = [piece("A", 48 * 16), piece("B", 48 * 16 - 2)];
        let cl = cut_list(&pieces, &stock(&[8], &[]), &Rules::default()).unwrap();
        assert_eq!(cl.boards.len(), 1);
        assert_eq!(cl.boards[0].offcut, Length::ZERO);
    }

    #[test]
    fn end_trim_comes_off_every_board() {
        let rules = Rules {
            end_trim: Length::inches(1),
            ..Rules::default()
        };
        let cl = cut_list(&[piece("A", 96 * 16)], &stock(&[8, 10], &[]), &rules).unwrap();
        assert_eq!(cl.boards[0].length, Length::feet(10));
    }

    #[test]
    fn picks_the_cheaper_mix() {
        // Two 70" pieces: one 12' board ($8) beats two 8' boards ($10).
        let pieces = [piece("A", 70 * 16), piece("B", 70 * 16)];
        let cl = cut_list(&pieces, &stock(&[8, 12], &[500, 800]), &Rules::default()).unwrap();
        assert_eq!(cl.boards.len(), 1);
        assert_eq!(cl.boards[0].length, Length::feet(12));
        assert_eq!(cl.summary().total_cents, Some(800));

        // Once the 12' board costs more than two 8' boards, buy the pair.
        let cl = cut_list(&pieces, &stock(&[8, 12], &[500, 1100]), &Rules::default()).unwrap();
        assert_eq!(cl.boards.len(), 2);
        assert_eq!(cl.summary().total_cents, Some(1000));
    }

    #[test]
    fn reports_pieces_longer_than_stock() {
        let pieces = [piece("TP1", 18 * 192), piece("S1", 92 * 16)];
        let cl = cut_list(&pieces, &stock(&[8, 16], &[]), &Rules::default()).unwrap();
        assert_eq!(cl.too_long.len(), 1);
        assert_eq!(cl.too_long[0].label, "TP1");
        assert_eq!(cl.boards.len(), 1);
    }

    #[test]
    fn sizes_without_stock_use_defaults() {
        let mut header = piece("H1", 41 * 16 + 8);
        header.size = Nominal::new(2, 8);
        let cl = cut_list(
            &[header.clone(), header],
            &stock(&[8], &[]),
            &Rules::default(),
        )
        .unwrap();
        assert_eq!(cl.boards.len(), 1);
        assert_eq!(cl.boards[0].id, "2x8-01");
        assert_eq!(cl.boards[0].length, Length::feet(8));
        assert_eq!(cl.summary().total_cents, None);
    }

    #[test]
    fn bom_groups_and_totals() {
        let pieces: Vec<_> = (0..5)
            .map(|i| piece(&format!("S{i}"), 115 * 16 + 8))
            .collect();
        let cl = cut_list(&pieces, &stock(&[8, 10], &[498, 698]), &Rules::default()).unwrap();
        let bom = cl.bom();
        assert_eq!(bom.len(), 1);
        assert_eq!(bom[0].count, 5);
        assert_eq!(bom[0].length, Length::feet(10));
        assert_eq!(bom[0].total_cents, Some(5 * 698));
    }

    #[test]
    fn bad_stock_is_an_error() {
        let rules = Rules::default();
        assert!(matches!(
            cut_list(&[], &stock(&[8, 10], &[498]), &rules),
            Err(CutListError::PriceCount { .. })
        ));
        assert!(matches!(
            cut_list(&[], &stock(&[], &[]), &rules),
            Err(CutListError::NoLengths { .. })
        ));
    }

    #[test]
    fn the_whole_shed_needs_fewer_boards_than_one_per_piece() {
        let project = Project::from_toml(include_str!("../../../examples/shed.toml")).unwrap();
        let framed = sticks_rules::frame_project(&project).unwrap();
        assert!(framed.errors().is_empty());
        let members = framed.members();
        let cl = cut_list_with_sheets(
            &members,
            &framed.sheet_pieces(),
            &project.stock,
            &project.sheets,
            &project.rules,
        )
        .unwrap();
        assert_accounted_for(&members, &cl);
        // Plates, rims, beams and fascia are split, so everything fits stock.
        assert!(cl.too_long.is_empty(), "{:?}", cl.too_long);
        // 252 sq ft of floor is at least eight 4x8 sheets.
        let subfloor = cl
            .sheets
            .iter()
            .filter(|s| s.thickness == Length::from_sixteenths(12))
            .count();
        assert!((8..=10).contains(&subfloor), "{subfloor}");
        // Walls and roofs are sheathed from their own stock.
        assert!(cl.sheets.len() > subfloor + 20, "{}", cl.sheets.len());
        let s = cl.summary();
        assert!(
            s.boards < s.pieces,
            "{} boards for {} pieces",
            s.boards,
            s.pieces
        );
    }

    fn assert_accounted_for(members: &[Member], cl: &CutList) {
        let mut want: Vec<_> = members.iter().map(|m| m.label.clone()).collect();
        let mut got: Vec<_> = cl
            .boards
            .iter()
            .flat_map(|b| &b.pieces)
            .chain(&cl.too_long)
            .map(|m| m.label.clone())
            .collect();
        want.sort();
        got.sort();
        assert_eq!(got, want, "every piece exactly once");
        for b in &cl.boards {
            assert!(!b.pieces.is_empty());
            assert!(
                b.used(cl.kerf, cl.end_trim) <= b.length,
                "{} overfull",
                b.id
            );
            assert!(b.pieces.iter().all(|p| p.size == b.size));
        }
    }

    proptest! {
        #[test]
        fn every_piece_once_and_no_board_overfull(
            lengths in prop::collection::vec(1i64..=20 * 192, 0..60),
            sizes in prop::collection::vec(0usize..2, 60),
            kerf in 0i64..=4,
            trim in 0i64..=16,
            feet in prop::sample::subsequence(vec![6i64, 8, 10, 12, 16], 1..=5),
            priced in any::<bool>(),
        ) {
            let members: Vec<_> = lengths
                .iter()
                .enumerate()
                .map(|(i, &l)| {
                    let mut m = piece(&format!("P{i:02}"), l);
                    m.size = [Nominal::new(2, 4), Nominal::new(2, 6)][sizes[i]];
                    m
                })
                .collect();
            let prices: Vec<u32> = if priced {
                feet.iter().map(|&f| (f * 70 + 50) as u32).collect()
            } else {
                Vec::new()
            };
            let rules = Rules {
                kerf: Length::from_sixteenths(kerf),
                end_trim: Length::from_sixteenths(trim),
                ..Rules::default()
            };
            let cl = cut_list(&members, &stock(&feet, &prices), &rules).unwrap();
            assert_accounted_for(&members, &cl);
            prop_assert!(cl.summary().boards <= cl.summary().pieces);
        }
    }
}
