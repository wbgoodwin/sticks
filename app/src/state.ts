// The project being edited, kept in local storage between sessions.

import type { OpeningJson, OpeningKind, ProjectJson } from "./core";

const KEY = "sticks.project.v1";

/** The front wall of the reference shed from the PRD. */
export function defaultProject(): ProjectJson {
  return {
    name: "Backyard shed",
    rules: { on_center: '16"', wall_lumber: "2x4", layout_from: "left" },
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
