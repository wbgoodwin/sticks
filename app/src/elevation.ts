// Draws one wall's elevation as SVG, seen from outside, with an overall
// length, the height, and the layout marks you'd snap on the plates.

import { formatLength, type MemberKind, type MemberView, type WallView } from "./core";
import type { Guide } from "./snap";

export interface DrawOptions {
  layoutFrom: "left" | "right";
  /** What's good about each opening's spot, such as "Centered on the wall". */
  notes?: string[][];
  drag?: DragOverlay;
}

/** What to show while an opening is dragged. */
export interface DragOverlay {
  index: number;
  guides: Guide[];
  /** The space to either side, measured while it moves. */
  gaps: [number, number][];
  /** Where the pointer is asking for, when the frame won't take it there. */
  ghost: { x: number; y: number; w: number; h: number } | null;
}

/** Maps the drawing back to the wall, in sixteenths. */
export interface Scale {
  /** Pixels per inch. */
  s: number;
  x16(px: number): number;
  y16(py: number): number;
}

export const KIND_NAMES: Record<MemberKind, string> = {
  bottom_plate: "Bottom plate",
  top_plate: "Top plate",
  cap_plate: "Cap plate",
  stud: "Stud",
  king_stud: "King stud",
  jack_stud: "Jack stud",
  cripple: "Cripple",
  header: "Header",
  sill: "Sill",
  blocking: "Blocking",
  rim_joist: "Rim joist",
  joist: "Joist",
  beam: "Beam",
  skid: "Skid",
  ledger: "Ledger",
  rafter: "Rafter",
  fascia: "Fascia",
  post: "Post",
  ridge: "Ridge board",
  collar_tie: "Collar tie",
  gable_stud: "Gable stud",
  decking: "Decking",
};

const SVG = "http://www.w3.org/2000/svg";

function el<K extends keyof SVGElementTagNameMap>(
  tag: K,
  attrs: Record<string, string | number>,
  text?: string,
): SVGElementTagNameMap[K] {
  const node = document.createElementNS(SVG, tag);
  for (const [k, v] of Object.entries(attrs)) node.setAttribute(k, String(v));
  if (text !== undefined) node.textContent = text;
  return node;
}

export function groupKey(m: MemberView): string {
  return `${m.kind}|${m.size}|${m.length_16}`;
}

// Layout marks under the bottom plate: rotated labels in rows, each row
// tall enough for the longest label, and each label one line across.
const MARK_ROW = 76;
const MARK_GAP = 16;
const MARK_ROWS = 2;
/** Height of the leader's jog over to a label nudged clear of its neighbor. */
const MARK_JOG = 14;
const PAD = { left: 72, right: 64, top: 64 };
/** Room under the wall for `rows` rows of marks and the layout note. */
const padBottom = (rows: number) => 16 + MARK_JOG + MARK_ROW * rows + 16;

/**
 * The row for each mark, at `xs` in order across: the first row with room,
 * or once all `rowCount` are taken, the one whose last mark is farthest away.
 */
function markRows(xs: number[], rowCount: number): number[] {
  const rowEnds: number[] = [];
  return xs.map((x) => {
    let row = rowEnds.findIndex((end) => Math.abs(x - end) >= MARK_GAP);
    if (row === -1 && rowEnds.length < rowCount) row = rowEnds.length;
    if (row === -1) {
      row = 0;
      rowEnds.forEach((end, i) => {
        if (Math.abs(x - end) > Math.abs(x - rowEnds[row])) row = i;
      });
    }
    rowEnds[row] = x;
    return row;
  });
}

/**
 * Where to put labels for marks at `xs` (in order across) so none overlap:
 * runs of marks too close together spread out, centered on where they are.
 */
function spreadLabels(xs: number[]): number[] {
  const runs: { xs: number[]; start: number }[] = [];
  for (const x of xs) {
    let run = { xs: [x], start: x };
    // Merge back into the run before while the two would overlap.
    for (;;) {
      const prev = runs[runs.length - 1];
      if (!prev || prev.start + (prev.xs.length - 1) * MARK_GAP + MARK_GAP <= run.start) break;
      runs.pop();
      const all = [...prev.xs, ...run.xs];
      const mid = all.reduce((sum, v) => sum + v, 0) / all.length;
      run = { xs: all, start: mid - ((all.length - 1) * MARK_GAP) / 2 };
    }
    runs.push(run);
  }
  return runs.flatMap((run) => run.xs.map((_, i) => run.start + i * MARK_GAP));
}

export function drawElevation(
  svg: SVGSVGElement,
  wall: WallView,
  options: DrawOptions,
  width: number,
  height: number,
): Scale {
  svg.setAttribute("viewBox", `0 0 ${width} ${height}`);
  svg.replaceChildren();
  const L = wall.length_16 / 16;
  const H = wall.height_16 / 16;
  const availW = width - PAD.left - PAD.right;

  // Where each layout mark goes across depends on the scale, and the scale
  // on how many rows of marks need room, so settle the two together.
  const marked = [...wall.marks]
    .sort((a, b) => a.at_16 - b.at_16)
    .flatMap((m) => {
      const member = wall.members.find((v) => v.label === m.label);
      if (!member) return [];
      return [{ mark: m, edge16: options.layoutFrom === "left" ? member.x_16 : member.x_16 + member.w_16 }];
    });
  // Use the fewest rows that give every mark room, since each row costs the
  // wall some height. Marks still too close get nudged apart below.
  const fit = (rowCount: number) => {
    const availH = height - PAD.top - padBottom(rowCount);
    const s = Math.max(0.1, Math.min(availW / L, availH / H));
    const xs = marked.map((m) => (m.edge16 / 16) * s);
    const rows = markRows(xs, rowCount);
    const crowded = rows.some((r, i) => rows.slice(0, i).some((q, j) => q === r && xs[i] - xs[j] < MARK_GAP));
    return { rowCount, s, rows, crowded };
  };
  let layout = fit(1);
  while (layout.crowded && layout.rowCount < MARK_ROWS) layout = fit(layout.rowCount + 1);
  const { rowCount, s, rows } = layout;
  const availH = height - PAD.top - padBottom(rowCount);
  const x0 = PAD.left + (availW - L * s) / 2;
  const yb = PAD.top + (availH - H * s) / 2 + H * s;
  const X = (x16: number) => x0 + (x16 / 16) * s;
  const Y = (y16: number) => yb - (y16 / 16) * s;

  // Openings first so framing draws over their outlines.
  const drag = options.drag;
  const openings = el("g", { class: "openings" });
  const handles = el("g", { class: "opening-handles" });
  for (const [i, o] of wall.openings.entries()) {
    const x = X(o.x_16);
    const y = Y(o.y_16 + o.h_16);
    const w = (o.w_16 / 16) * s;
    const hgt = (o.h_16 / 16) * s;
    const notes = options.notes?.[i] ?? [];
    const dragged = drag?.index === i;
    openings.append(
      el("rect", {
        class: `rough-opening${dragged ? " dragging" : ""}${notes.length > 0 ? " ideal" : ""}`,
        x,
        y,
        width: w,
        height: hgt,
      }),
    );
    // A clear target over the hole itself, above the framing, to grab it by.
    handles.append(el("rect", { class: "opening-handle", "data-index": i, x, y, width: w, height: hgt }));
    // Labels only where they fit inside the opening (about 7px a character).
    const size = `${formatLength(o.w_16, true)} × ${formatLength(o.h_16, true)}`;
    const cx = x + w / 2;
    const cy = y + hgt / 2;
    if (hgt > 40 && w > o.name.length * 7 + 12) {
      openings.append(el("text", { class: "opening-name", x: cx, y: cy - 4 }, o.name));
      if (w > size.length * 7 + 12) {
        openings.append(el("text", { class: "opening-size", x: cx, y: cy + 12 }, size));
      }
    }
    // The good thing about its spot, or a check mark where the words won't fit.
    if (notes.length > 0 && !dragged) {
      const text = shortNote(notes[0]);
      const fits = hgt > 64 && w > text.length * 6.5 + 16;
      const mark = el("text", { class: "opening-ideal", x: cx, y: fits ? cy + 30 : y + Math.min(14, hgt / 2 + 4) }, fits ? `✓ ${text}` : "✓");
      mark.append(el("title", {}, notes.join("\n")));
      openings.append(mark);
    }
  }
  svg.append(openings);

  // Members. Header plies share a rectangle, so draw each spot once.
  const members = el("g", { class: "members" });
  const plies = new Map<string, number>();
  for (const m of wall.members) {
    const spot = `${m.x_16},${m.y_16},${m.w_16},${m.h_16}`;
    plies.set(spot, (plies.get(spot) ?? 0) + 1);
  }
  const drawn = new Set<string>();
  for (const m of wall.members) {
    const spot = `${m.x_16},${m.y_16},${m.w_16},${m.h_16}`;
    if (drawn.has(spot)) continue;
    drawn.add(spot);
    const data = {
      class: `member kind-${m.kind}`,
      "data-label": m.label,
      "data-group": groupKey(m),
      "data-plies": plies.get(spot) ?? 1,
    };
    // Sloped plates and studs with an angled top come with their corners.
    const shape =
      m.outline_16.length > 0
        ? el("polygon", { ...data, points: m.outline_16.map(([x, y]) => `${X(x)},${Y(y)}`).join(" ") })
        : el("rect", {
            ...data,
            x: X(m.x_16),
            y: Y(m.y_16 + m.h_16),
            width: Math.max(1, (m.w_16 / 16) * s),
            height: Math.max(1, (m.h_16 / 16) * s),
          });
    members.append(shape);
  }
  svg.append(members);

  if (drag) {
    const g = el("g", { class: "drag-overlay" });
    for (const guide of drag.guides) {
      g.append(
        guide.dir === "v"
          ? el("line", { class: "snap-guide", x1: X(guide.at), y1: Y(wall.height_16) - 14, x2: X(guide.at), y2: Y(0) + 8 })
          : el("line", { class: "snap-guide", x1: X(0) - 8, y1: Y(guide.at), x2: X(wall.length_16) + 8, y2: Y(guide.at) }),
      );
    }
    const o = wall.openings[drag.index];
    // Below the opening's name, so the two don't overlap.
    const gy = Y(o.y_16 + o.h_16 * 0.3);
    const even = drag.gaps.length === 2 && drag.gaps[0][1] - drag.gaps[0][0] === drag.gaps[1][1] - drag.gaps[1][0];
    for (const [a, b] of drag.gaps) {
      if (b - a < 16) continue;
      const d = dimension(X(a), gy, X(b), gy, formatLength(b - a), "h");
      d.classList.add("gap", ...(even ? ["even"] : []));
      g.append(d);
    }
    if (drag.ghost) {
      const r = drag.ghost;
      g.append(
        el("rect", {
          class: "drag-ghost",
          x: X(r.x),
          y: Y(r.y + r.h),
          width: (r.w / 16) * s,
          height: (r.h / 16) * s,
        }),
      );
    }
    svg.append(g);
  }
  svg.append(handles);

  // Plate splices: a red line across each piece that starts at the joint.
  const splices = el("g", { class: "splices" });
  for (const sp of wall.splices) {
    for (const m of wall.members.filter((v) => v.kind === sp.kind && v.x_16 === sp.at_16)) {
      const ys = m.outline_16.length > 0 ? m.outline_16.filter(([x]) => x === sp.at_16).map(([, y]) => y) : [m.y_16, m.y_16 + m.h_16];
      const x = X(sp.at_16);
      splices.append(
        el("line", { class: "splice", x1: x, y1: Y(Math.min(...ys)) + 4, x2: x, y2: Y(Math.max(...ys)) - 4 }),
      );
    }
    const label = el("text", { class: "splice-text", x: X(sp.at_16), y: sp.kind === "bottom_plate" ? Y(0) + 14 : Y(wall.height_16) - 8 }, "splice");
    label.append(el("title", {}, `${KIND_NAMES[sp.kind]} splice at ${sp.at}, centered on a stud`));
    splices.append(label);
  }
  svg.append(splices);

  // Overall length, above the wall.
  const top = Y(wall.height_16) - 28;
  svg.append(dimension(X(0), top, X(wall.length_16), top, formatLength(wall.length_16), "h"));

  // Height, left of the wall, and at the right too when the top slopes.
  const left = X(0) - 32;
  svg.append(dimension(left, Y(0), left, Y(wall.left_height_16), formatLength(wall.left_height_16), "v"));
  if (wall.right_height_16 !== wall.left_height_16) {
    const right = X(wall.length_16) + 32;
    svg.append(dimension(right, Y(0), right, Y(wall.right_height_16), formatLength(wall.right_height_16), "v"));
  }

  // Layout marks along the bottom plate, crowded marks dropping to the next
  // row and nudged clear of each other with a jog in their leader. Every
  // label gets the same weight and a halo so it reads over the grid.
  const marks = el("g", { class: "marks" });
  for (let row = 0; row < rowCount; row++) {
    const inRow = marked.filter((_, i) => rows[i] === row);
    const xs = inRow.map((m) => X(m.edge16));
    const at = spreadLabels(xs);
    const ty = yb + 16 + MARK_JOG + row * MARK_ROW;
    inRow.forEach(({ mark: m }, i) => {
      const x = xs[i];
      const lx = at[i];
      marks.append(
        el("polyline", {
          class: "tick",
          points: `${x},${yb + 3} ${x},${ty - MARK_JOG} ${lx},${ty - 4}`,
        }),
        el(
          "text",
          {
            class: "mark-text",
            x: lx,
            y: ty,
            dy: "0.35em",
            transform: `rotate(-90 ${lx} ${ty})`,
            "data-label": m.label,
            "data-kind": m.kind,
          },
          m.at,
        ),
      );
    });
  }
  svg.append(marks);

  const start = options.layoutFrom === "left" ? X(0) : X(wall.length_16);
  svg.append(
    el(
      "text",
      {
        class: "layout-origin",
        x: start,
        y: height - 12,
        "text-anchor": options.layoutFrom === "left" ? "start" : "end",
      },
      options.layoutFrom === "left" ? "Layout measured from this end →" : "← Layout measured from this end",
    ),
  );

  return {
    s,
    x16: (px) => ((px - x0) / s) * 16,
    y16: (py) => ((yb - py) / s) * 16,
  };
}

/** A few words for a note, to fit inside the opening. */
function shortNote(note: string): string {
  if (note.startsWith("Centered between")) return "Centered in its space";
  if (note.startsWith("Centered")) return "Centered";
  if (note.startsWith("Spaced evenly")) return "Evenly spaced";
  return note.replace(/ with .*/, "");
}

function dimension(
  x1: number,
  y1: number,
  x2: number,
  y2: number,
  text: string,
  dir: "h" | "v",
): SVGGElement {
  const g = el("g", { class: "dimension" });
  g.append(el("line", { x1, y1, x2, y2 }));
  const tick = 6;
  for (const [x, y] of [
    [x1, y1],
    [x2, y2],
  ]) {
    g.append(
      dir === "h"
        ? el("line", { x1: x, y1: y - tick, x2: x, y2: y + tick })
        : el("line", { x1: x - tick, y1: y, x2: x + tick, y2: y }),
    );
  }
  const mx = (x1 + x2) / 2;
  const my = (y1 + y2) / 2;
  const label =
    dir === "h"
      ? el("text", { x: mx, y: my - 8 }, text)
      : el("text", { x: mx - 8, y: my, transform: `rotate(-90 ${mx - 8} ${my})` }, text);
  g.append(label);
  return g;
}
