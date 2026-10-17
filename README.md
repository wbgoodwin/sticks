# Sticks

Turn a short description of a structure (a shed, a wall, a deck) into a framing
model, a lumber bill of materials, and a cut list that wastes as little lumber
as possible. Change one dimension and the frame, shopping list and cut list
regenerate.

The tool frames what you specify. It does not size beams, check spans, or
certify anything structurally.

## Status

Milestone M3 (floor and lean-to roof). One project file frames a whole
building: a floor on beams and piers, four outside walls generated from the
footprint, and a lean-to roof whose pitch sets the back and side wall
heights. From that come the shopping list (lumber and subfloor sheets), a
board-by-board cut list, and printable drawings of every wall, the floor, the
roof and a rafter.

```sh
cargo run -p sticks-cli -- frame examples/shed.toml               # every member, layout marks, splices
cargo run -p sticks-cli -- frame examples/shed.toml --part roof   # one wall, the floor or the roof
cargo run -p sticks-cli -- cuts examples/shed.toml                # shopping list and cut list
cargo run -p sticks-cli -- build examples/shed.toml --out build/  # CSVs, SVG drawings, print.html
```

`build` writes `bom.csv`, `cut-list.csv`, `wall-<id>.svg` for each wall,
`floor.svg`, `subfloor.svg`, `roof.svg`, `rafter.svg`, and `print.html`: the
whole build packet as one page to print, letter size, landscape.

## Project file

`examples/shed.toml` is the reference shed from the PRD. A `[building]` gives
the footprint and the front wall's height; `[floor]` and `[roof]` sit on it.

```toml
[building]
width = "18'"     # along the front wall
depth = "14'"
height = "10'"    # the front (high) wall

[floor]
joist = "2x6"
beam = "4x6"
beams = 3

[roof]
pitch = "3:12"

[[wall]]
id = "front"      # front, back, left and right take their size from the building
[[wall.opening]]
kind = "door"
# ...
```

Walls with other ids (an interior partition, say) give their own `length`,
`height` and ends, as before. A project with no `[building]` is just walls.

## The app

A desktop window (Tauri) that edits one wall and redraws its elevation, its
shopping list and its cut list as you type. Plate splices show in red. The
floor, roof and whole-building editing arrive in the app with M4. The same page runs in a browser,
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
- A plate longer than the longest stock board is spliced with its joint
  centered on a stud. Top plate and cap plate joints stay at least 24" apart.
  Bottom plates split the same way.

### Buildings

- The front and back walls run through the corners; the side walls fit
  between them. Seen from outside, each wall's left end is on your left, and
  openings are measured from it.
- Under a lean-to the front wall is the high wall. The back wall and the side
  walls take their heights from the pitch, so changing the pitch changes the
  walls. The side walls are rake walls: sloped top plates and studs with an
  angled top cut.

### Floor

- Joists run front to back on center from the layout end (flush end joists),
  between rim joists along the front and back. Bays between joists are listed
  so insulation can be cut to fit.
- Beams run along the front: the outer two under the rims, the rest evenly
  between, with a row of solid blocking over each inner beam. Piers are spaced
  evenly along each beam, no more than `pier_spacing` apart and `pier_inset`
  in from the ends. `support = "skids"` drops the piers.
- Rims and beams longer than any stock board are spliced over a joist or a
  pier; the plies of a built-up beam splice over different piers.
- The subfloor runs across the joists in rows, every other row starting with
  a half sheet, so sheet ends land on joists.

### Lean-to roof

- Rafters sit on center from the layout end, flush at both ends. Each rests
  on the front wall's cap plate at its outside face and the back wall's at its
  inside face, with a birdsmouth over each wall: a level seat the depth of the
  wall and a plumb heel. Both tails are cut plumb.
- Overhangs are level distances past the walls (default 12"). Fascia (default
  one size deeper than the rafters, so it covers the plumb cut) runs across
  both tails and splices over a rafter.
- Rake overhangs at the sides, a ledger against an existing wall, and roof
  sheathing aren't framed yet.

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
- A piece longer than the longest stock is never split by the cut list. The
  framing splits plates, rims, beams and fascia over their supports; anything
  else that's too long (a rafter, say) is reported so you can add longer stock.
- Subfloor sheets are packed in shelves on whole sheets. No kerf is taken
  between sheet pieces, so an edge piece may run a blade's width short.

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
