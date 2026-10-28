# Sticks

Turn a short description of a structure (a shed, a wall, a deck) into a framing
model, a lumber bill of materials, and a cut list that wastes as little lumber
as possible. Change one dimension and the frame, shopping list and cut list
regenerate.

The tool frames what you specify. It does not size beams, check spans, or
certify anything structurally.

## Status

Milestone M5 (porch, sheathing, gable). The whole shed can be designed in the
app, in a browser or on the desktop; see [The app](#the-app). One project file
frames a whole building: a floor on beams and piers, four outside walls
generated from the footprint, a lean-to or gable roof, a porch roof on posts
off the front with a deck under it, and sheathing on the walls and roof. From
that come the shopping list (lumber, sheets, and the Simpson Strong-Tie
connectors with their nails), a board-by-board cut list,
and printable drawings of every wall, the floor, the roof, its rafters, the
porch, the deck and the sheathing layout.

```sh
cargo run -p sticks-cli -- frame examples/shed.toml               # every member, layout marks, splices
cargo run -p sticks-cli -- frame examples/shed.toml --part porch  # one wall, floor, roof, porch or sheathing
cargo run -p sticks-cli -- cuts examples/shed.toml                # shopping list and cut list
cargo run -p sticks-cli -- build examples/shed.toml --out build/  # CSVs, SVG drawings, print.html
```

`build` writes `bom.csv`, `cut-list.csv`, `hardware.csv`, `wall-<id>.svg` for each wall,
`floor.svg`, `subfloor.svg`, `roof.svg`, `rafter.svg` (and `rafter-2.svg`
when the back rafters differ), `porch.svg`, `porch-rafter.svg`, `deck.svg`,
`wall-sheathing.svg`, `roof-sheathing.svg`, and `print.html`: the whole build
packet as one page to print, letter size, landscape. Parts the project
doesn't have are skipped.

### Changes in plain English (prototype)

```sh
cargo run -p sticks-cli -- edit examples/shed.toml "add a window" --questions
# a classifier answers them into answers.json, as {"0.target": {"pick": "new_opening", "confidence": 0.98}}
cargo run -p sticks-cli -- edit examples/shed.toml "add a window" --answers answers.json --questions
# ...and again until --questions prints [], then
cargo run -p sticks-cli -- edit examples/shed.toml "add a window" --answers answers.json --answer 0.wall=back --write
```

Rust reads only the notation: it splits the request into clauses and reads
lengths (`20 ft`, `3' 6"`), pitches (`4 in 12`) and sizes (`12x16`, `2x6`).
The meaning comes from a classifier answering multiple-choice questions that
Rust builds from the design. First, what does each clause change? The options
are every part of the design that can change, with its value now ("The front
wall's door, 3' 2-1/2" x 6' 10-1/2", 7' 3" from the left end"), plus a new
opening. Then that target's own questions: what to do to it (set it, bigger
or smaller, resize, move, remove), where (centered, an end, beside another
opening, one on each side, spread out), how many, and what each number is
for. `--questions` prints what the classifier should answer next, given
`--answers` so far, so a request takes two rounds. Questions the request may
not answer, such as which wall, also offer "the request doesn't say".
`edit` turns answers at 80% confidence or more into typed edits
(`sticks-intent`). Anything less sure, or not in the request, comes back as a
question for you, answered with `--answer ID=LABEL`. Rust then places the
edits, sliding an opening to the nearest spot that frames and giving a door
up to 4' wide a 2x6 header if that's what fits under the top plates. It
frames the result and shows the change in boards and sheets.
`--json` prints the same for a program, and `--write` saves once it frames
(comments in the file are not kept).

In the app, the bar above the drawing takes the same requests (Ctrl K or
⌘K). The bar is off unless the app is started with `VITE_JEV=on` or is set up for Sticks accounts (below). It shows each change, what was assumed, and buttons for anything left to
ask, along with the change in boards and sheets. Nothing changes until you
press Make, and Undo puts it back. The app asks
[Jev](https://docs.typesafe.ai/introduction/quickstart) through the dev
server, which adds your key so the page never holds it. Start it with the key
set: `VITE_JEV=on TYPESAFE_API_KEY=... npm run tauri dev` (or `npm run dev`),
or put both in `app/.env.local`. Signed in to a Sticks account on the paid
plan, the app asks Jev through the account server instead, which holds the
key (see below), so a bundled build works too.

`app/scripts/jev-probe.ts` runs sample requests through Jev from the command
line: `TYPESAFE_API_KEY=... npm run probe:jev` in `app/`, which builds
`sticks` first.

## Project file

`examples/shed.toml` is the reference shed from the PRD. A `[building]` gives
the footprint and the front wall's height; `[floor]`, `[roof]`, `[porch]` and
`[sheathing]` build on it. A top-level `on_hand` list keeps stages you
already have off the shopping list, such as `on_hand = ["floor"]`. The stages
are `floor`, `subfloor`, `wall front` (and each other wall by id), `roof`,
`porch`, `deck`, `wall sheathing` and `roof sheathing`.

```toml
[building]
width = "18'"     # along the front wall
depth = "14'"
height = "10'"    # the front (high) wall

[floor]
joist = "2x6"
beam = "4x6"
beams = 3
support = "posts" # short posts on piers; or "piers", or "skids" on grade

[roof]
pitch = "2:12"    # kind = "gable" for a ridge over the middle

[porch]           # a porch roof on posts off the front wall
depth = "6'"
[porch.deck]      # decking on joists, a step below the shed floor
step_down = "1\""

[sheathing]
wall = "7/16\""
roof = "1/2\""

[[wall]]
id = "front"      # front, back, left and right take their size from the building
[[wall.opening]]
kind = "door"
# ...
```

Walls with other ids (an interior partition, say) give their own `length`,
`height` and ends, as before. A project with no `[building]` is just walls.

### Hardware

The framing rules note every joint that takes a connector (a rafter on a
plate, a joist hung on a ledger, a beam on a post), and the shopping list
carries a Simpson Strong-Tie part for each, sized to the lumber, in ZMAX
where it's outdoors or against treated wood, with boxes of the nails it
takes. Stages on hand take their hardware with them. The catalog is
`crates/sticks-hardware/src/simpson.toml`. Sticks picks the usual part for
the sizes; it doesn't check loads. An optional `[hardware]` section changes
what goes on the list:

```toml
[hardware]
fasten_with = "screws"        # SD connector screws in place of nails
high_wind = true              # every extra tie: ridge straps, joist ties, stud ties, deck ties
add = ["joist_on_beam"]       # or just some of them
[hardware.parts]
rafter_on_plate = "H1"        # a different model for a kind of joint, or "none"
[hardware.prices_cents]
"H2.5A" = 89                  # each, by model; nails and screws by the box
```

## The app

A desktop window (Tauri) for designing the whole shed. The panel on the left
sets the footprint, the front wall height, the roof shape, pitch and
overhangs, the floor's joists, beams and piers, the porch and its deck, the
sheathing, each wall's doors, windows and penetrations, and the lumber yard's
stock and prices. Everything redraws as you type:

- **3D**: every member, pier and sheet in place. Drag to orbit, scroll to
  zoom, hover a member to name it, click it for its size, length and cuts.
  Hide the sheathing, roof, walls, porch, subfloor or floor framing to see
  inside. Sheathing starts hidden so the framing shows.
- **Front, Back, Left, Right**: each wall's elevation with stud positions
  measured from the layout end. Plate splices show in red.
- **Floor, Subfloor, Roof, Rafter, Porch, Deck, Sheathing**: the printed
  framing plans, rafter details and sheet layouts.
- **Zooming the flat drawings**: pinch, Cmd with + − 0, Cmd with the scroll
  wheel, or the buttons in the corner, from 50% to 800%. The boards grow and the
  measurements keep their size. Zoomed in, scroll or drag the drawing to move
  around. Each drawing keeps its own zoom.
- **Shopping list and cut list** for the whole project, with the total when
  every stock length has a price, and the connectors and nails under it.
  Settings holds the hardware prices, nails or screws, and the high-wind
  switch.

**Print** opens the build packet (the shopping list, every drawing and the cut
list on letter pages) to print or download. **Open** and **Save** read and
write the TOML project file, and **New** starts from the reference shed or a
single wall. The app keeps your work between sessions.

The same page runs in a browser with nothing to install, since all framing
happens in the Rust core compiled to WebAssembly.

### Install

Sticks is installed by building it from this repository. You need
[Rust](https://rustup.rs), Node 22, and Tauri's prerequisites for your OS (on
a Mac, the Xcode command line tools: `xcode-select --install`). Then:

```sh
git clone https://github.com/wbgoodwin/sticks.git
cd sticks
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129
cd app
npm run app:install
```

On a Mac that builds `Sticks.app` and puts it in `/Applications`. On Windows
or Linux it builds the installers and prints where they are. The first build
takes a few minutes.

The app checks GitHub for a newer release once a day. When there is one, it
says so with a link to what's new. To update, run `git pull` and then
`npm run app:install` again in the `app` folder. Skipping a version keeps it
quiet until the next one.

To publish an update, raise `version` in `app/src-tauri/tauri.conf.json`,
merge it, and push a matching tag (`git tag v0.2.0 && git push origin
v0.2.0`). The Release workflow then publishes the GitHub release the app
looks for.

### Develop

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

### Sticks account

Without an account everything stays on this Mac. Signing in (Sign in, in the
top bar) and taking the paid plan ($5 a month) syncs designs
between Macs and turns on changes in plain English, which come with a
monthly allowance and then a small charge each, up to a limit you set. The
Account dialog shows the plan, this month's use, that limit, the day the plan
ends if you picked one, Stripe's billing page, and your synced designs.

The open project saves to the account two seconds after each change. If
another Mac saved the same design in between, this Mac's version is saved as
a copy next to it, so nothing is overwritten.

Accounts live in the private `sticks-yard` repo. Point the app at a deploy of
it in `app/.env.local`, using the deploy's outputs; without these the app has
no account features:

```sh
VITE_YARD_URL=https://....on.aws                                         # ApiUrl
VITE_YARD_SIGN_IN=https://sticks-yard.auth.us-east-1.amazoncognito.com   # SignInDomain
VITE_YARD_CLIENT_ID=...                                                  # ClientId
```

Sign-in opens in your browser. The desktop window waits for it on
`localhost:47615`; the browser build comes back to `/auth/callback`.

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
- Rake overhangs at the sides and a ledger against an existing wall aren't
  framed yet.

### Gable roof

- `kind = "gable"` puts a ridge board (default two sizes up from the rafters)
  over the middle of the depth, running along the front. Every wall is the
  front wall's height. Rafters pair up across the ridge, each with a plumb top
  cut against it and a birdsmouth over its wall.
- Collar ties (default 2x4) cross every pair about every 4', a third of the
  way down from the ridge.
- The side walls get gable studs from their cap plates up to the end rafters,
  over the wall's studs, plus one under the ridge.

### Porch

- The porch roof hangs from a ledger lagged to the front wall just under its
  plates, and slopes out (at the roof's pitch unless `pitch` is given) over a
  beam on posts. Rafters sit on center like the roof's, flush with the
  ledger's top, with a birdsmouth over the beam and a level `overhang` past it.
- Posts stand at both ends and evenly between, no more than `post_spacing`
  apart. A built-up beam's plies splice over different posts.
- A warning shows when the beam's underside is under 6' 8".
- `[porch.deck]` adds a deck: joists hung from a ledger on the floor's front
  rim, out to a rim over a beam on piers under the posts, with decking boards
  (default 5/4x6, 1/8" apart) along the front. Its top sits `step_down`
  (default 1") below the subfloor, and decking joints land on joists,
  staggered row to row. Without a deck the posts stand on piers.

### Sheathing

- Walls are sheathed from outside, sheets standing on end from the bottom of
  the floor's rim, with joints on studs, kings or gable studs. Door and window
  openings are cut out; the side walls follow the roof line.
- Roof sheets run along the eaves in rows up the slope, every other row
  starting with a half sheet so end joints stagger. The porch roof gets the
  same.
- Leave `wall` or `roof` out of `[sheathing]` to skip it.

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
- Subfloor and sheathing sheets are packed in shelves on whole sheets of
  their thickness. No kerf is taken
  between sheet pieces, so an edge piece may run a blade's width short.

## Layout

| Crate | Purpose |
| --- | --- |
| `sticks-core` | Lengths in integer 1/16", lumber sizes, members, the TOML project file |
| `sticks-rules` | Generators for floors, walls, openings, roofs, porches and sheathing |
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
