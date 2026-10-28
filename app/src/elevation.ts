// Draws one wall's elevation as SVG, seen from outside, with an overall
// length, the height, and the layout marks you'd snap on the plates.

import { formatLength, type MemberKind, type MemberView, type WallView } from "./core";
import type { Guide } from "./snap";

export interface DrawOptions {
  layoutFrom: "left" | "right";
  /** What's good about each opening's spot, such as "Centered on the wall". */
  notes?: string[][];
  drag?: DragOverlay;
  /** A plate picked out: its marks replace the stud row, kings and jacks included. */
  plate?: "bottom_plate" | "top_plate";
  /**
   * How much bigger than fitting `width` by `height` to draw the wall. The
   * labels and the room for them stay the same size.
   */
  zoom?: number;
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
  /** The drawing's size, which zoomed isn't the size it was asked to fit. */
  width: number;
  height: number;
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

// Under the bottom plate: one row of stud positions, and below it a bracket
// for each opening's rough opening. Kings and jacks aren't numbered: they go
// just outside each rough opening edge. Labels are rotated so studs on
// layout never crowd, each reading up from the end of its tick.
const MARK_GAP = 16;
const LABEL_LONG = 64;
const STUD_ROW = 14 + LABEL_LONG + 14;
const OPENING_ROW = 14 + LABEL_LONG + 8;
const PAD = { left: 72, right: 64, top: 64 };
/** Room under the wall for the marks and the layout note. */
const padBottom = (openings: boolean) => STUD_ROW + (openings ? OPENING_ROW : 0) + 24;

/**
 * Where to put labels for marks at `xs` (in order across) so none overlap.
 * Studs on layout are far enough apart to sit right at their mark; the odd
 * run too close together, like a corner, spreads out centered on itself.
 */
function spreadLabels(xs: number[]): number[] {
  const runs: { xs: number[]; start: number }[] = [];
  for (const x of xs) {
    let run = { xs: [x], start: x };
    for (;;) {
      const prev = runs[runs.length - 1];
      if (!prev || prev.start + prev.xs.length * MARK_GAP <= run.start) break;
      runs.pop();
      const all = [...prev.xs, ...run.xs];
      const mid = all.reduce((sum, v) => sum + v, 0) / all.length;
      run = { xs: all, start: mid - ((all.length - 1) * MARK_GAP) / 2 };
    }
    runs.push(run);
  }
  return runs.flatMap((run) => run.xs.map((_, i) => run.start + i * MARK_GAP));
}

/** A rotated label reading up, its top at (`x`, `y`). */
function upLabel(x: number, y: number, text: string, attrs: Record<string, string> = {}): SVGTextElement {
  return el(
    "text",
    { class: "mark-text", x, y, dy: "0.35em", transform: `rotate(-90 ${x} ${y})`, ...attrs },
    text,
  );
}

export function drawElevation(
  svg: SVGSVGElement,
  wall: WallView,
  options: DrawOptions,
  width: number,
  height: number,
): Scale {
  svg.replaceChildren();
  const L = wall.length_16 / 16;
  const H = wall.height_16 / 16;
  const padY = PAD.top + padBottom(wall.openings.length > 0);
  const zoom = options.zoom ?? 1;
  const s = Math.max(0.1, Math.min((width - PAD.left - PAD.right) / L, (height - padY) / H) * zoom);
  // Zoomed out, the drawing is just the wall and its labels; zoomed in, at
  // least the stage.
  const fill = zoom >= 1;
  width = Math.max(L * s + PAD.left + PAD.right, fill ? width : 0);
  height = Math.max(H * s + padY, fill ? height : 0);
  svg.setAttribute("viewBox", `0 0 ${width} ${height}`);
  const availW = width - PAD.left - PAD.right;
  const availH = height - padY;
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

  // Stud positions along the bottom plate. With a plate picked out, every
  // board on that plate instead, each with its letter: X, K, J or C.
  const marks = el("g", { class: options.plate ? "marks plate-marks" : "marks" });
  const near = (label: string) => {
    const member = wall.members.find((v) => v.label === label);
    return member ? X(options.layoutFrom === "left" ? member.x_16 : member.x_16 + member.w_16) : null;
  };
  const studs = options.plate
    ? (wall.plates?.marks ?? []).flatMap((m) => {
        const top = options.plate === "top_plate";
        const b = top ? m.top : m.bottom;
        const x = b && near(b.label);
        return b && x !== null ? [{ label: b.label, text: `${b.letter}  ${top ? m.top_at : m.at}`, x }] : [];
      })
    : [...wall.marks]
        .sort((a, b) => a.at_16 - b.at_16)
        .flatMap((m) => {
          if (m.kind === "king_stud" || m.kind === "jack_stud") return [];
          const x = near(m.label);
          return x === null ? [] : [{ label: m.label, text: m.at, x }];
        });
  const at = spreadLabels(studs.map((v) => v.x));
  const ty = yb + 16;
  studs.forEach(({ label, text, x }, i) => {
    marks.append(
      el("polyline", { class: "tick", points: `${x},${yb + 3} ${x},${ty - 8} ${at[i]},${ty - 3}` }),
      upLabel(at[i], ty, text, { "data-label": label }),
    );
  });

  // Each opening's rough opening as a bracket, its edges measured from the
  // layout end like the studs.
  const by = yb + STUD_ROW + 8;
  for (const o of wall.openings) {
    const a = X(o.x_16);
    const b = X(o.x_16 + o.w_16);
    const measure = (x16: number) =>
      formatLength(options.layoutFrom === "left" ? x16 : wall.length_16 - x16, true);
    marks.append(
      el("line", { class: "ro", x1: a, y1: by, x2: b, y2: by }),
      el("line", { class: "ro", x1: a, y1: by - 6, x2: a, y2: by + 6 }),
      el("line", { class: "ro", x1: b, y1: by - 6, x2: b, y2: by + 6 }),
      upLabel(a, by + 10, measure(o.x_16)),
      upLabel(b, by + 10, measure(o.x_16 + o.w_16)),
    );
    // The name only where it fits over the bracket.
    if (b - a >= o.name.length * 7 + 8) {
      marks.append(el("text", { class: "ro-name", x: (a + b) / 2, y: by - 8 }, o.name));
    }
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
      originNote(wall, options),
    ),
  );

  return {
    s,
    width,
    height,
    x16: (px) => ((px - x0) / s) * 16,
    y16: (py) => ((yb - py) / s) * 16,
  };
}

/** The note under the drawing: where the layout starts, and what the marks are. */
function originNote(wall: WallView, options: DrawOptions): string {
  const left = options.layoutFrom === "left";
  const what = options.plate
    ? `${options.plate === "top_plate" ? "Top" : "Bottom"} plate marks${options.plate === "top_plate" && wall.plates?.top_sloped ? ", along its slope" : ""}: X stud, K king, J jack, C cripple`
    : wall.openings.length
      ? "Brackets: rough openings"
      : "";
  const arrow = left ? "Layout measured from this end →" : "← Layout measured from this end";
  if (!what) return arrow;
  return left ? `${arrow}   ${what}` : `${what}   ${arrow}`;
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
