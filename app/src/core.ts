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
  | "post"
  | "ridge"
  | "collar_tie"
  | "gable_stud"
  | "decking";

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

/** A rough opening's outline in its wall's elevation. */
export interface OpeningView {
  name: string;
  x_16: number;
  y_16: number;
  w_16: number;
  h_16: number;
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
  openings: OpeningView[];
  warnings: string[];
  error: string | null;
}

/** The floor or roof framing in plan. */
export interface PlanView {
  width_16: number;
  depth_16: number;
  members: MemberView[];
  warnings: string[];
  error: string | null;
}

/** Sheathing panels, cut out around doors and windows. */
export interface SheathingView {
  pieces: number;
  warnings: string[];
  error: string | null;
}

/** A printable plan drawing besides the walls. */
export interface DrawingView {
  /** What `drawing` takes, such as `roof` or `porch-rafter`. */
  id: string;
  name: string;
  /** The part it shows: `floor`, `roof`, `porch` or `sheathing`. */
  part: "floor" | "roof" | "porch" | "sheathing";
}

export interface FrameView {
  walls: WallView[];
  floor: PlanView | null;
  roof: PlanView | null;
  /** The porch roof, posts and beam, and its deck when it has one. */
  porch: PlanView | null;
  sheathing: SheathingView | null;
  drawings: DrawingView[];
}

export type SolidKind = MemberKind | "pier" | "subfloor" | "sheathing";

/** A prism: `profile` in the u, v plane from `origin`, pushed `depth` along u × v. Inches. */
export interface SolidView {
  label: string;
  /** A wall id, `floor`, `roof`, `porch` or `deck`. */
  part: string;
  kind: SolidKind;
  size: string | null;
  length: string | null;
  cuts: string[];
  profile: [number, number][];
  origin: [number, number, number];
  u: [number, number, number];
  v: [number, number, number];
  depth: number;
}

export interface ModelView {
  solids: SolidView[];
  bounds: [[number, number, number], [number, number, number]] | null;
  skipped: string[];
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
    end_trim?: string;
    corner_style?: string;
  };
  stock?: StockJson[];
  sheet?: SheetJson[];
  building?: BuildingJson;
  floor?: FloorJson;
  roof?: RoofJson;
  porch?: PorchJson;
  sheathing?: SheathingJson;
  wall: WallJson[];
}

export interface BuildingJson {
  /** Along the front wall. */
  width: string;
  depth: string;
  /** The front wall; under a lean-to the others follow the pitch. */
  height: string;
}

export interface FloorJson {
  joist: string;
  on_center?: string;
  beam: string;
  beam_plies: number;
  beams: number;
  support: "piers" | "skids";
  pier_spacing: string;
  pier_inset: string;
  subfloor: string;
}

export interface RoofJson {
  kind?: "lean_to" | "gable";
  pitch: string;
  rafter: string;
  on_center?: string;
  front_overhang: string;
  back_overhang: string;
  fascia?: string;
  /** Gable only. */
  ridge?: string;
  collar_tie?: string;
}

/** A porch roof on posts off the front wall. */
export interface PorchJson {
  depth?: string;
  post?: string;
  post_spacing?: string;
  beam?: string;
  beam_plies?: number;
  rafter?: string;
  pitch?: string;
  overhang?: string;
  ledger?: string;
  ledger_height?: string;
  deck?: DeckJson;
}

/** Decking on joists that hang from the floor's rim. */
export interface DeckJson {
  joist?: string;
  beam?: string;
  decking?: string;
  gap?: string;
  /** How far the deck's top sits below the subfloor's. */
  step_down?: string;
}

/** Sheet thicknesses; a part left out isn't sheathed. */
export interface SheathingJson {
  wall?: string;
  roof?: string;
}

export interface SheetJson {
  thickness: string;
  length?: string;
  width?: string;
  price_cents?: number;
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

/**
 * A wall. The building's own walls (front, back, left, right) take their
 * length, height and ends from the building and leave those out.
 */
export interface WallJson {
  id: string;
  length?: string;
  height?: string;
  left_end?: WallEnd;
  right_end?: WallEnd;
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

/** Throws when the project as a whole doesn't hang together. */
export function frame(project: ProjectJson): FrameView {
  return JSON.parse(wasm.frame(JSON.stringify(forCore(project))));
}

/** Throws when the stock can't be read (an unparseable length, say). */
export function cutList(project: ProjectJson): CutListView {
  return JSON.parse(wasm.cut_list(JSON.stringify(forCore(project))));
}

export function model(project: ProjectJson): ModelView {
  return JSON.parse(wasm.model3d(JSON.stringify(forCore(project))));
}

/** A printable drawing as SVG: a wall id or a `DrawingView` id. */
export function drawing(project: ProjectJson, which: string): string {
  return wasm.drawing(JSON.stringify(forCore(project)), which);
}

/** The whole build packet as one HTML page. */
export function printPacket(project: ProjectJson): string {
  return wasm.print_packet(JSON.stringify(forCore(project)));
}

/** Reads a project file. Throws with the reason when it can't. */
export function projectFromToml(text: string): ProjectJson {
  return JSON.parse(wasm.project_from_toml(text));
}

export function projectToToml(project: ProjectJson): string {
  return wasm.project_to_toml(JSON.stringify(forCore(project)));
}

/** A multiple-choice question about a request, for Jev to answer. */
export interface EditAsk {
  type: "choice";
  id: string;
  /** The clause of the request it's about. */
  text: string;
  prompt: string;
  options: EditOption[];
}

export interface EditOption {
  label: string;
  description: string;
}

/** An answer by question id: Jev's pick and how sure it was, or the user's. */
export interface EditAnswer {
  pick: string;
  confidence: number;
  by_user?: boolean;
}

/** A question only the user can settle. */
export interface EditQuestion {
  id: string;
  text: string;
  prompt: string;
  options: EditOption[];
  /** The request doesn't say, or Jev's best guess and how sure it was. */
  reason: string;
}

/** One clause of a request and what it came to. */
export interface EditStep {
  text: string;
  /** Each change in builder English. */
  changes: string[];
  assumed: string[];
  question: EditQuestion | null;
  problem: string | null;
}

export interface EditReading {
  steps: EditStep[];
  /** The project with every change made, once nothing is left to ask. */
  project: ProjectJson | null;
  /** Why the changed project won't frame, if it won't. */
  problems: string[];
}

/** The design in a few lines, the context Jev reads with each request. */
export function describeDesign(project: ProjectJson): string {
  return wasm.describe_design(JSON.stringify(forCore(project)));
}

/** Every question about a request, for Jev to answer in one call. */
export function editQuestions(project: ProjectJson, request: string): EditAsk[] {
  return JSON.parse(wasm.edit_questions(JSON.stringify(forCore(project)), request));
}

/** What a request comes to with the answers so far. */
export function readEdit(project: ProjectJson, request: string, answers: Record<string, EditAnswer>): EditReading {
  return JSON.parse(wasm.read_edit(JSON.stringify(forCore(project)), request, JSON.stringify(answers)));
}

/**
 * The project as the core reads it: partly priced stock counts as unpriced,
 * and a building's walls with nothing on them are left out.
 */
function forCore(project: ProjectJson): ProjectJson {
  const stock = project.stock?.map((s) => ({
    ...s,
    prices_cents: s.prices_cents.every((p) => p !== null) ? s.prices_cents : [],
  }));
  const wall = project.building
    ? project.wall.filter((w) => !isSide(w.id) || w.opening.length > 0 || w.tees.length > 0)
    : project.wall;
  return { ...project, stock, wall };
}

export const SIDES = ["front", "back", "left", "right"] as const;

/** Whether a wall id names a side of the building. */
export function isSide(id: string): boolean {
  return (SIDES as readonly string[]).includes(id);
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
