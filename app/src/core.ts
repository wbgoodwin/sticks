// Typed access to the Rust core, compiled to WebAssembly.

import init, * as wasm from "./wasm/sticks_wasm.js";

export type MemberKind =
  | "bottom_plate"
  | "top_plate"
  | "cap_plate"
  | "stud"
  | "king_stud"
  | "jack_stud"
  | "cripple"
  | "header"
  | "sill"
  | "blocking"
  | "rim_joist"
  | "joist"
  | "beam"
  | "skid"
  | "ledger"
  | "rafter"
  | "fascia"
  | "post";

export interface MemberView {
  label: string;
  kind: MemberKind;
  size: string;
  length_16: number;
  length: string;
  x_16: number;
  y_16: number;
  w_16: number;
  h_16: number;
  /** Corners as [x, y] for sloped members; empty when the rectangle is the shape. */
  outline_16: [number, number][];
}

/** A plate joint, centered on a stud. */
export interface SpliceView {
  kind: MemberKind;
  at_16: number;
  at: string;
}

export interface MarkView {
  label: string;
  kind: MemberKind;
  at_16: number;
  at: string;
}

export interface WallView {
  id: string;
  length_16: number;
  height_16: number;
  left_height_16: number;
  right_height_16: number;
  members: MemberView[];
  marks: MarkView[];
  splices: SpliceView[];
  warnings: string[];
  error: string | null;
}

export type OpeningKind = "door" | "window" | "penetration";
export type WallEnd = "plain" | "corner";

export interface PieceView {
  label: string;
  kind: MemberKind;
  size: string;
  length_16: number;
  length: string;
}

export interface BoardView {
  id: string;
  size: string;
  length_16: number;
  length: string;
  price_cents: number | null;
  pieces: PieceView[];
  offcut_16: number;
  offcut: string;
}

export interface BomLineView {
  size: string;
  length_16: number;
  length: string;
  count: number;
  unit_price_cents: number | null;
  total_cents: number | null;
}

export interface CutListView {
  boards: BoardView[];
  bom: BomLineView[];
  too_long: PieceView[];
  summary: {
    boards: number;
    sheets: number;
    pieces: number;
    total_cents: number | null;
    waste_percent: number;
    kerf: string;
  };
  /** Parts that don't frame, such as `wall front` or `floor`. */
  skipped: string[];
}

/** The project file's shape. Lengths stay as the builder typed them. */
export interface ProjectJson {
  name: string;
  rules: {
    on_center: string;
    wall_lumber: string;
    layout_from: "left" | "right";
    kerf?: string;
  };
  stock?: StockJson[];
  wall: WallJson[];
}

/**
 * Boards the yard sells in one size. The app allows a blank price while one
 * is being typed; the core needs a price for every length or none.
 */
export interface StockJson {
  size: string;
  lengths: string[];
  prices_cents: (number | null)[];
}

export interface WallJson {
  id: string;
  length: string;
  height: string;
  left_end: WallEnd;
  right_end: WallEnd;
  tees: string[];
  opening: OpeningJson[];
}

export interface OpeningJson {
  kind: OpeningKind;
  offset: string;
  width: string;
  height: string;
  sill_height: string;
  header: string;
}

export async function loadCore(): Promise<void> {
  await init();
}

export function frame(project: ProjectJson): WallView[] {
  return JSON.parse(wasm.frame(JSON.stringify(forCore(project)))).walls;
}

/** Throws when the stock can't be read (an unparseable length, say). */
export function cutList(project: ProjectJson): CutListView {
  return JSON.parse(wasm.cut_list(JSON.stringify(forCore(project))));
}

/** The project as the core reads it: partly priced stock counts as unpriced. */
function forCore(project: ProjectJson): ProjectJson {
  const stock = project.stock?.map((s) => ({
    ...s,
    prices_cents: s.prices_cents.every((p) => p !== null) ? s.prices_cents : [],
  }));
  return { ...project, stock };
}

/** Sixteenths of an inch, or the reason the text isn't a length. */
export function parseLength(text: string): { ok: number } | { error: string } {
  try {
    return { ok: wasm.parse_length(text) };
  } catch (e) {
    return { error: e instanceof Error ? e.message : String(e) };
  }
}

export function formatLength(sixteenths: number, inchesOnly = false): string {
  return wasm.format_length(sixteenths, inchesOnly);
}
