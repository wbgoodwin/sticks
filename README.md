# Sticks

Turn a short description of a structure (a shed, a wall, a deck) into a framing
model, a lumber bill of materials, and a cut list that wastes as little lumber
as possible. Change one dimension and the frame, shopping list and cut list
regenerate.

The tool frames what you specify. It does not size beams, check spans, or
certify anything structurally.

## Status

Milestone M2 (cut list). The core frames stud walls: plates, studs on center
from either end, three-stud corners, partition tees, doors, windows and small
penetrations, with layout marks for build day. It then plans which stock
boards to buy and how to cut every piece from them.

```sh
cargo run -p sticks-cli -- frame examples/shed.toml   # members and layout marks
cargo run -p sticks-cli -- cuts examples/shed.toml    # shopping list and cut list
cargo run -p sticks-cli -- build examples/shed.toml --out build/   # bom.csv, cut-list.csv
```

## The app

A desktop window (Tauri) that edits one wall and redraws its elevation, its
shopping list and its cut list as you type. The same page runs in a browser,
since all framing happens in the Rust core compiled to WebAssembly.

You need Node 22, the `wasm32-unknown-unknown` Rust target, and the
`wasm-bindgen` CLI at the version pinned in `crates/sticks-wasm/Cargo.toml`:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129
cd app
npm install
npm run dev          # in a browser at http://localhost:5173
npm run tauri dev    # as a desktop app
```

The desktop build also needs Tauri's system prerequisites for your OS (on
Linux, the WebKitGTK development packages); see
https://v2.tauri.app/start/prerequisites/.

## Framing conventions

These are the defaults; each one is a setting in the project file.

- Studs are laid out on center from the chosen end. The first common stud's
  near edge sits at 15-1/4" for 16" spacing, so sheet edges land on stud
  centers.
- Wall height is floor to top of the cap plate. Studs are that height less
  three plates.
- Openings are rough-opening sizes, positioned from the wall's left end. Each
  gets a king and a jack stud per side, a built-up header sized by you (two
  plies in a 2x4 wall), cripples on layout above it, and for windows a flat
  sill with cripples below. Doors at floor level cut the bottom plate out.
- A penetration that misses every stud is framed with blocks above and below;
  one that hits a stud is framed like a small window.
- A `corner` end is the wall that runs through: end stud, 1-1/2" spacer
  blocks, then a backer stud. The butting wall's end stud is the third stud.
- A partition tee puts a stud on each side of the partition.

## Cut list

For each lumber size, pieces are sorted longest first and each goes on the open
board it leaves the least room on; when none fits, a new board is opened. The
plan is tried a few ways (open the cheapest length that fits, or favor each
stock length in turn) and the cheapest wins, then each board is cut down to
the cheapest stock length that still holds its pieces.

- Every cut loses the saw kerf (default 1/8"), and `end_trim` comes off each
  board to square the factory end (default 0"). The last piece on a board may
  run to its end.
- With a price on every stock length of a size, the plan minimizes cost;
  without prices it minimizes lumber bought.
- A size with no `[[stock]]` entry is bought in 8', 10', 12', 14' and 16'
  lengths.
- A piece longer than the longest stock is never split by the cut list. It is
  reported instead, until plate splitting over studs arrives in M3.

## Layout

| Crate | Purpose |
| --- | --- |
| `sticks-core` | Lengths in integer 1/16", lumber sizes, members, the TOML project file |
| `sticks-rules` | Generators for floors, walls, openings and roofs |
| `sticks-cutlist` | Bill of materials and the cut-list optimizer |
| `sticks-export` | CSV, SVG elevations, printable pages |
| `sticks-cli` | The `sticks` command |
| `sticks-wasm` | The core compiled to WebAssembly for the app |
| `app/` | The TypeScript app and its Tauri desktop shell |

The desktop and web app is a TypeScript UI (three.js arrives with the 3D view
in M4), packaged with Tauri on the desktop. Both run the same Rust core
compiled to WebAssembly, so the desktop app, the web app and the CLI give
identical results.

## Milestones

| | Scope |
| --- | --- |
| M0 | Fixed-point units, fraction parsing, member and project types, TOML loading |
| M1 | Walls: plates, studs, corners, openings, plus a first app window showing one elevation |
| M2 | Cut list v1 (best-fit decreasing, kerf, stock prices), BOM, CSV |
| M3 | Floor, lean-to roof, plate splitting, SVG elevations |
| M4 | 3D view, print packet, web build |
| M5 | Porch, sheathing, gable roof |

## Lengths

Project files accept builder notation: `10'`, `10' 3-1/2"`, `92-5/8"`, `3.5`
(a bare number is inches). Everything is stored as whole sixteenths of an inch,
so sums never drift.

## License

MIT
