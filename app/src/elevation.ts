// Draws one wall's elevation as SVG, seen from outside, with an overall
// length, the height, and the layout marks you'd snap on the plates.

import { formatLength, type MemberKind, type MemberView, type WallView } from "./core";

export interface OpeningOutline {
  name: string;
  x16: number;
  y16: number;
  w16: number;
  h16: number;
}

export interface DrawOptions {
  layoutFrom: "left" | "right";
  openings: OpeningOutline[];
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

const MARK_ROW = 54;
const MARK_ROWS = 2;
const MARK_GAP = 13;
const PAD = { left: 72, right: 32, top: 64, bottom: 16 + MARK_ROW * MARK_ROWS + 28 };

export function drawElevation(
  svg: SVGSVGElement,
  wall: WallView,
  options: DrawOptions,
  width: number,
  height: number,
): void {
  svg.setAttribute("viewBox", `0 0 ${width} ${height}`);
  svg.replaceChildren();
  const L = wall.length_16 / 16;
  const H = wall.height_16 / 16;
  const availW = width - PAD.left - PAD.right;
  const availH = height - PAD.top - PAD.bottom;
  const s = Math.max(0.1, Math.min(availW / L, availH / H));
  const x0 = PAD.left + (availW - L * s) / 2;
  const yb = PAD.top + (availH - H * s) / 2 + H * s;
  const X = (x16: number) => x0 + (x16 / 16) * s;
  const Y = (y16: number) => yb - (y16 / 16) * s;

  // Openings first so framing draws over their outlines.
  const openings = el("g", { class: "openings" });
  for (const o of options.openings) {
    const x = X(o.x16);
    const y = Y(o.y16 + o.h16);
    const w = (o.w16 / 16) * s;
    const hgt = (o.h16 / 16) * s;
    openings.append(el("rect", { class: "rough-opening", x, y, width: w, height: hgt }));
    // Labels only where they fit inside the opening (about 7px a character).
    const size = `${formatLength(o.w16, true)} × ${formatLength(o.h16, true)}`;
    const cx = x + w / 2;
    const cy = y + hgt / 2;
    if (hgt > 40 && w > o.name.length * 7 + 12) {
      openings.append(el("text", { class: "opening-name", x: cx, y: cy - 4 }, o.name));
      if (w > size.length * 7 + 12) {
        openings.append(el("text", { class: "opening-size", x: cx, y: cy + 12 }, size));
      }
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
    const rect = el("rect", {
      class: `member kind-${m.kind}`,
      x: X(m.x_16),
      y: Y(m.y_16 + m.h_16),
      width: Math.max(1, (m.w_16 / 16) * s),
      height: Math.max(1, (m.h_16 / 16) * s),
      "data-label": m.label,
      "data-group": groupKey(m),
      "data-plies": plies.get(spot) ?? 1,
    });
    members.append(rect);
  }
  svg.append(members);

  // Overall length, above the wall.
  const top = Y(wall.height_16) - 28;
  svg.append(dimension(X(0), top, X(wall.length_16), top, formatLength(wall.length_16), "h"));

  // Height, left of the wall.
  const left = X(0) - 32;
  svg.append(dimension(left, Y(0), left, Y(wall.height_16), formatLength(wall.height_16), "v"));

  // Layout marks along the bottom plate. Close marks drop to a second row.
  const marks = el("g", { class: "marks" });
  const rowEnds: number[] = [];
  for (const m of [...wall.marks].sort((a, b) => a.at_16 - b.at_16)) {
    const member = wall.members.find((v) => v.label === m.label);
    if (!member) continue;
    const edge16 = options.layoutFrom === "left" ? member.x_16 : member.x_16 + member.w_16;
    const x = X(edge16);
    // Rotated labels need about one line height of horizontal room.
    let row = rowEnds.findIndex((end) => Math.abs(x - end) >= MARK_GAP);
    if (row === -1) row = rowEnds.length;
    if (row >= MARK_ROWS) continue;
    rowEnds[row] = x;
    const ty = yb + 16 + row * MARK_ROW;
    marks.append(
      el("line", { class: `tick kind-${m.kind}`, x1: x, y1: yb + 3, x2: x, y2: ty - 3 }),
      el(
        "text",
        {
          class: `mark-text kind-${m.kind}`,
          x,
          y: ty,
          transform: `rotate(-90 ${x} ${ty})`,
          "data-label": m.label,
        },
        m.at,
      ),
    );
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
