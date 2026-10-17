// The project being edited, kept in local storage between sessions.

import shedToml from "../../examples/shed.toml?raw";
import { isSide, projectFromToml, type OpeningJson, type OpeningKind, type ProjectJson, type StockJson } from "./core";

const KEY = "sticks.project.v2";

export type Template = "shed" | "wall";

export const TEMPLATES: [Template, string][] = [
  ["shed", "Reference shed"],
  ["wall", "Single wall"],
];

/** A fresh project: the reference shed from the PRD, or one wall with a door. */
export function fromTemplate(template: Template): ProjectJson {
  if (template === "shed") return normalize(projectFromToml(shedToml));
  return normalize({
    name: "Partition wall",
    rules: { on_center: '16"', wall_lumber: "2x4", layout_from: "left", kerf: '1/8"' },
    stock: [{ size: "2x4", lengths: ["8'", "10'", "12'", "16'"], prices_cents: [] }],
    wall: [
      {
        id: "partition",
        length: "12'",
        height: "97-1/8\"",
        left_end: "plain",
        right_end: "plain",
        tees: [],
        opening: [newOpening("door", "3'")],
      },
    ],
  });
}

/** Fills in what the project file leaves out, so the panel can edit it in place. */
export function normalize(project: ProjectJson): ProjectJson {
  project.wall ??= [];
  for (const w of project.wall) {
    w.opening ??= [];
    w.tees ??= [];
    for (const o of w.opening) o.sill_height ??= "0";
    if (!project.building || !isSide(w.id)) {
      w.left_end ??= "plain";
      w.right_end ??= "plain";
    }
  }
  for (const s of project.stock ?? []) s.prices_cents ??= [];
  project.rules.layout_from ??= "left";
  return project;
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

/** The fascia the core uses when the roof names none: two sizes up from the rafter. */
export function fascia(rafter: string): string {
  const [t, w] = rafter.split("x");
  return `${t}x${Number(w) + 2}`;
}

/** Every lumber size the frame cuts. */
export function sizesUsed(project: ProjectJson): string[] {
  const sizes = [project.rules.wall_lumber, ...project.wall.flatMap((w) => w.opening.map((o) => o.header))];
  if (project.floor) sizes.push(project.floor.joist, project.floor.beam);
  if (project.roof) sizes.push(project.roof.rafter, project.roof.fascia ?? fascia(project.roof.rafter));
  if (project.porch) {
    const p = project.porch;
    sizes.push(p.post ?? "4x4", p.beam ?? "2x8");
    if (p.deck) sizes.push(p.deck.decking ?? "5/4x6", p.deck.beam ?? "4x6");
  }
  return [...new Set(sizes)];
}

/**
 * Adds stock for any lumber size the frame uses but the yard list lacks, and
 * sheets for the subfloor and sheathing, so everything can be priced.
 * `framed` adds the sizes of members the core framed, such as a porch's
 * defaults. Returns whether anything was added.
 */
export function ensureStock(project: ProjectJson, framed: string[] = []): boolean {
  project.stock ??= [];
  const stock = project.stock;
  let added = false;
  for (const size of new Set([...sizesUsed(project), ...framed])) {
    if (!stock.some((s) => s.size === size)) {
      stock.push(newStock(size));
      added = true;
    }
  }
  if (added) stock.sort((a, b) => sizeOrder(a.size) - sizeOrder(b.size));
  const thicknesses = [project.floor?.subfloor, project.sheathing?.wall, project.sheathing?.roof];
  for (const thickness of thicknesses) {
    if (!thickness) continue;
    project.sheet ??= [];
    if (!project.sheet.some((s) => s.thickness === thickness)) {
      project.sheet.push({ thickness, length: "8'", width: "4'" });
      added = true;
    }
  }
  return added;
}

/** Thin boards first, then narrow: 5/4x6 sorts between 1x and 2x. */
function sizeOrder(size: string): number {
  const [t, w] = size.split("x");
  const [n, d] = t.split("/").map(Number);
  return (d ? n / d : n) * 100 + Number(w);
}

/** The saved project, or null when there is none or it can't be read. */
export function load(): ProjectJson | null {
  try {
    const saved = localStorage.getItem(KEY);
    if (saved) return normalize(JSON.parse(saved));
  } catch {
    // Unreadable storage just means starting fresh.
  }
  return null;
}

export function save(project: ProjectJson): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(project));
  } catch {
    // Saving is a convenience; the project file is the real record.
  }
}
