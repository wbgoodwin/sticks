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
  | "blocking";

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
  members: MemberView[];
  marks: MarkView[];
  warnings: string[];
  error: string | null;
}

export type OpeningKind = "door" | "window" | "penetration";
export type WallEnd = "plain" | "corner";

/** The project file's shape. Lengths stay as the builder typed them. */
export interface ProjectJson {
  name: string;
  rules: {
    on_center: string;
    wall_lumber: string;
    layout_from: "left" | "right";
  };
  wall: WallJson[];
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
  return JSON.parse(wasm.frame(JSON.stringify(project))).walls;
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
