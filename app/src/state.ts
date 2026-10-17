// The project being edited, kept in local storage between sessions.

import type { OpeningJson, OpeningKind, ProjectJson, StockJson } from "./core";

const KEY = "sticks.project.v1";

/** The front wall of the reference shed from the PRD. */
export function defaultProject(): ProjectJson {
  return {
    name: "Backyard shed",
    rules: { on_center: '16"', wall_lumber: "2x4", layout_from: "left", kerf: '1/8"' },
    stock: [
      { size: "2x4", lengths: ["8'", "10'", "12'", "16'"], prices_cents: [] },
      newStock("2x8"),
    ],
    wall: [
      {
        id: "front",
        length: "18'",
        height: "10'",
        left_end: "corner",
        right_end: "corner",
        tees: [],
        opening: [
          {
            kind: "door",
            offset: "7' 3\"",
            width: '38-1/2"',
            height: '82-1/2"',
            sill_height: "0",
            header: "2x8",
          },
          {
            kind: "penetration",
            offset: "15'",
            width: '4"',
            height: '4"',
            sill_height: "1'",
            header: "2x4",
          },
        ],
      },
    ],
  };
}

export function newOpening(kind: OpeningKind, offset: string): OpeningJson {
  switch (kind) {
    case "door":
      return { kind, offset, width: '38-1/2"', height: '82-1/2"', sill_height: "0", header: "2x8" };
    case "window":
      return { kind, offset, width: '36"', height: '36"', sill_height: '44"', header: "2x6" };
    case "penetration":
      return { kind, offset, width: '4"', height: '4"', sill_height: "1'", header: "2x4" };
  }
}

/** What a yard usually carries when the project doesn't say. */
export function newStock(size: string): StockJson {
  return { size, lengths: ["8'", "10'", "12'", "14'", "16'"], prices_cents: [] };
}

/**
 * Adds stock for any lumber size the frame uses but the yard list lacks, so
 * every size can be priced. Returns whether anything was added.
 */
export function ensureStock(project: ProjectJson): boolean {
  project.stock ??= [];
  const stock = project.stock;
  const used = [project.rules.wall_lumber, ...project.wall.flatMap((w) => w.opening.map((o) => o.header))];
  let added = false;
  for (const size of used) {
    if (!stock.some((s) => s.size === size)) {
      stock.push(newStock(size));
      added = true;
    }
  }
  if (added) stock.sort((a, b) => sizeOrder(a.size) - sizeOrder(b.size));
  return added;
}

function sizeOrder(size: string): number {
  const [t, w] = size.split("x").map(Number);
  return t * 100 + w;
}

export function load(): ProjectJson {
  try {
    const saved = localStorage.getItem(KEY);
    if (saved) return JSON.parse(saved);
  } catch {
    // Unreadable storage just means starting fresh.
  }
  return defaultProject();
}

export function save(project: ProjectJson): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(project));
  } catch {
    // Saving is a convenience; the project file is the real record.
  }
}
