# Sticks

Turn a short description of a structure (a shed, a wall, a deck) into a framing
model, a lumber bill of materials, and a cut list that wastes as little lumber
as possible. Change one dimension and the frame, shopping list and cut list
regenerate.

The tool frames what you specify. It does not size beams, check spans, or
certify anything structurally.

## Status

Early work on milestone M0 (units and model). Today you can load a project
file and check it:

```sh
cargo run -p sticks-cli -- check examples/shed.toml
```

## Layout

| Crate | Purpose |
| --- | --- |
| `sticks-core` | Lengths in integer 1/16", lumber sizes, members, the TOML project file |
| `sticks-rules` | Generators for floors, walls, openings and roofs |
| `sticks-cutlist` | Bill of materials and the cut-list optimizer |
| `sticks-export` | CSV, SVG elevations, printable pages |
| `sticks-cli` | The `sticks` command |

The desktop and web app will be a TypeScript UI with three.js for the 3D view,
packaged with Tauri on the desktop. Both run the same Rust core compiled to
WebAssembly, so the desktop app, the web app and the CLI give identical results.

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
