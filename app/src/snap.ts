// Spots where an opening sits well on its wall, for snapping while it's
// dragged and for marking it once it's there: centered on the wall, centered
// in the space beside its neighbors, spaced like a neighbor, or with its
// head or sill in line with another opening's.

import type { OpeningKind } from "./core";

/** One opening on the wall, in sixteenths, seen from outside. */
export interface Item {
  name: string;
  kind: OpeningKind;
  x: number;
  y: number;
  w: number;
  h: number;
}

/** A line across the wall: vertical at an x, or level at a height. */
export interface Guide {
  dir: "v" | "h";
  at: number;
}

/** Something good about where an opening is. */
export interface Fact {
  text: string;
  guides: Guide[];
}

export interface Spot {
  x: number;
  y: number;
}

/** Within a sixteenth counts as on the spot, for centers that land between sixteenths. */
const TOLERANCE = 1;

interface Candidate {
  at: number;
  fact: Fact;
}

/** What's to either side of the opening: the nearest other opening or the wall end. */
export interface Bounds {
  left: number;
  right: number;
  leftItem: Item | null;
  rightItem: Item | null;
}

/** Penetrations are too small to line anything up with. */
function references(items: Item[], index: number): Item[] {
  return items.filter((o, i) => i !== index && o.kind !== "penetration");
}

export function bounds(items: Item[], index: number, x: number, length: number): Bounds {
  const me = items[index];
  const center = x + me.w / 2;
  const b: Bounds = { left: 0, right: length, leftItem: null, rightItem: null };
  for (const o of references(items, index)) {
    if (o.x + o.w / 2 < center) {
      if (o.x + o.w >= b.left) Object.assign(b, { left: o.x + o.w, leftItem: o });
    } else if (o.x <= b.right) {
      Object.assign(b, { right: o.x, rightItem: o });
    }
  }
  return b;
}

function candidatesX(items: Item[], index: number, x: number, length: number): Candidate[] {
  const me = items[index];
  const out: Candidate[] = [
    { at: Math.round((length - me.w) / 2), fact: { text: "Centered on the wall", guides: [{ dir: "v", at: length / 2 }] } },
  ];
  const b = bounds(items, index, x, length);
  if (b.leftItem || b.rightItem) {
    const left = b.leftItem ? b.leftItem.name : "the left end";
    const right = b.rightItem ? b.rightItem.name : "the right end";
    const mid = (b.left + b.right) / 2;
    out.push({ at: Math.round(mid - me.w / 2), fact: { text: `Centered between ${left} and ${right}`, guides: [{ dir: "v", at: mid }] } });
  }
  // The same gap a neighbor has on its far side, so a row of windows spaces evenly.
  const others = items.map((o, i) => (i === index ? { ...o, x: -1e9 } : o));
  if (b.leftItem) {
    const n = b.leftItem;
    const gap = n.x - bounds(others, items.indexOf(n), n.x, length).left;
    out.push({ at: n.x + n.w + gap, fact: { text: `Spaced evenly with ${n.name}`, guides: [] } });
  }
  if (b.rightItem) {
    const n = b.rightItem;
    const gap = bounds(others, items.indexOf(n), n.x, length).right - (n.x + n.w);
    out.push({ at: n.x - gap - me.w, fact: { text: `Spaced evenly with ${n.name}`, guides: [] } });
  }
  return out;
}

function candidatesY(items: Item[], index: number): Candidate[] {
  const me = items[index];
  const out: Candidate[] = [];
  for (const o of references(items, index)) {
    const head = o.y + o.h;
    out.push({ at: head - me.h, fact: { text: `Head in line with ${o.name}`, guides: [{ dir: "h", at: head }] } });
    if (o.kind === "window" && me.kind !== "door") {
      out.push({ at: o.y, fact: { text: `Sill in line with ${o.name}`, guides: [{ dir: "h", at: o.y }] } });
    }
  }
  return out;
}

/** Doors stand on the floor; everything else can move up and down. */
export function movesUpAndDown(kind: OpeningKind): boolean {
  return kind !== "door";
}

/** The good things about an opening's spot, each said once. */
export function factsAt(items: Item[], index: number, spot: Spot, length: number): Fact[] {
  const hits = [
    ...candidatesX(items, index, spot.x, length).filter((c) => Math.abs(c.at - spot.x) <= TOLERANCE),
    ...candidatesY(items, index).filter((c) => Math.abs(c.at - spot.y) <= TOLERANCE),
  ];
  const seen = new Set<string>();
  return hits.map((c) => c.fact).filter((f) => !seen.has(f.text) && seen.add(f.text));
}

/**
 * Where a dragged opening lands: the nearest good spot within `reach`
 * sixteenths, else the nearest `grid`. A `reach` of zero places it freely.
 */
export function snap(items: Item[], index: number, raw: Spot, length: number, reach: number, grid: number): Spot {
  const pick = (value: number, candidates: Candidate[]) => {
    let best: number | null = null;
    for (const c of candidates) {
      const d = Math.abs(c.at - value);
      if (d <= reach && (best === null || d < Math.abs(best - value))) best = c.at;
    }
    return best ?? Math.round(value / grid) * grid;
  };
  const me = items[index];
  const x = Math.min(Math.max(0, pick(raw.x, candidatesX(items, index, raw.x, length))), length - me.w);
  const y = movesUpAndDown(me.kind) ? Math.max(0, pick(raw.y, candidatesY(items, index))) : me.y;
  return { x, y };
}
