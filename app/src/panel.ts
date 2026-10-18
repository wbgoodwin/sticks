// The parameter panel: every field edits the project in place and asks for a
// re-frame. The panel is rebuilt only when its structure changes (an opening
// added, the roof removed, another wall picked), so typing never loses focus.
// A building's panel has two tabs: the building's parts, each folding to a
// one-line summary, and the walls' openings and partitions.

import {
  isSide,
  parseLength,
  type BuildingJson,
  type OpeningKind,
  type PorchJson,
  type ProjectJson,
  type RoofJson,
  type WallJson,
} from "./core";
import { newOpening } from "./state";

export interface PanelEvents {
  /** A value changed; re-frame. */
  changed(): void;
  /** Something was added or removed; rebuild the panel too. */
  restructured(): void;
}

type Attrs = Record<string, string | boolean | ((e: Event) => void)>;

export function h<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  attrs: Attrs = {},
  ...children: (Node | string | null)[]
): HTMLElementTagNameMap[K] {
  const el = document.createElement(tag);
  for (const [key, value] of Object.entries(attrs)) {
    if (typeof value === "function") el.addEventListener(key.slice(2), value);
    else if (typeof value === "boolean") value && el.setAttribute(key, "");
    else el.setAttribute(key, value);
  }
  for (const child of children) if (child !== null) el.append(child);
  return el;
}

/** A text field that accepts builder notation and flags anything it can't read. */
export function lengthField(
  label: string,
  get: () => string,
  set: (v: string) => void,
  events: PanelEvents,
  hint?: string,
): HTMLElement {
  const message = h("div", { class: "field-error", role: "alert" });
  const input = h("input", {
    type: "text",
    value: get(),
    spellcheck: "false",
    autocomplete: "off",
    "aria-label": label,
    oninput: () => {
      const result = parseLength(input.value);
      if ("error" in result) {
        input.classList.add("invalid");
        message.textContent = "Try a length like 92-5/8\" or 10' 3\"";
        return;
      }
      input.classList.remove("invalid");
      message.textContent = "";
      set(input.value);
      events.changed();
    },
  });
  return h(
    "label",
    { class: "field" },
    h("span", { class: "field-label" }, label),
    input,
    hint ? h("span", { class: "field-hint" }, hint) : null,
    message,
  );
}

function segmented<T extends string>(
  label: string,
  options: [T, string][],
  get: () => T,
  set: (v: T) => void,
  onChange: () => void,
): HTMLElement {
  const group = h("div", { class: "segmented", role: "radiogroup", "aria-label": label });
  const buttons = options.map(([value, text]) => {
    const b = h(
      "button",
      {
        type: "button",
        role: "radio",
        "aria-checked": String(get() === value),
        onclick: () => {
          set(value);
          for (const other of buttons) other.setAttribute("aria-checked", String(other === b));
          onChange();
        },
      },
      text,
    );
    return b;
  });
  group.append(...buttons);
  return h("div", { class: "field" }, h("span", { class: "field-label" }, label), group);
}

function select(
  label: string,
  options: string[],
  get: () => string,
  set: (v: string) => void,
  events: PanelEvents,
): HTMLElement {
  const el = h(
    "select",
    {
      "aria-label": label,
      onchange: () => {
        set(el.value);
        events.changed();
      },
    },
    ...(options.includes(get()) ? options : [get(), ...options]).map((o) =>
      h("option", { value: o, selected: o === get() }, o),
    ),
  );
  return h("label", { class: "field" }, h("span", { class: "field-label" }, label), el);
}

const KIND_NAMES: Record<OpeningKind, string> = {
  door: "Door",
  window: "Window",
  penetration: "Penetration",
};

const HEADERS = ["2x4", "2x6", "2x8", "2x10", "2x12"];
const RAFTERS = ["2x4", "2x6", "2x8", "2x10", "2x12"];
const JOISTS = ["2x6", "2x8", "2x10", "2x12"];
const BEAMS = ["4x4", "4x6", "4x8", "6x6", "2x8", "2x10", "2x12"];
const POSTS = ["4x4", "6x6"];
const PORCH_BEAMS = ["2x6", "2x8", "2x10", "2x12", "4x6", "4x8"];
const DECKING = ["5/4x6", "2x6", "2x4"];
const SHEATHING = ['3/8"', '7/16"', '1/2"', '5/8"', '3/4"'];
const NO_SHEATHING = "None";

const WALL_NAMES: Record<string, string> = { front: "Front", back: "Back", left: "Left", right: "Right" };

export function wallName(id: string): string {
  return WALL_NAMES[id] ?? id.charAt(0).toUpperCase() + id.slice(1);
}

/** The panel's two tabs, when the project is a building. */
export type PanelTab = "building" | "walls";

/** What the panel needs to know about the framed project. */
export interface PanelContext {
  /** Every wall, in drawing order. */
  wallIds: string[];
  /** The wall whose openings are being edited. */
  wall: string;
  selectWall(id: string): void;
  /** A wall's framed length in sixteenths, when it frames. */
  wallLength(id: string): number | null;
  /** The tab on show. */
  tab: PanelTab;
  selectTab(tab: PanelTab): void;
}

export function renderPanel(
  root: HTMLElement,
  project: ProjectJson,
  ctx: PanelContext,
  events: PanelEvents,
): void {
  const building = project.building;
  if (!building) {
    root.replaceChildren(wallsSection(project, ctx, events));
    return;
  }
  const tabs = segmented(
    "Design",
    [
      ["building", "Building"],
      ["walls", "Walls"],
    ],
    () => ctx.tab,
    (v) => ctx.selectTab(v),
    () => {},
  );
  // The group's own label names it; the tabs need no heading.
  tabs.querySelector(".field-label")?.remove();
  tabs.classList.add("panel-tabs");
  root.replaceChildren(
    tabs,
    ...(ctx.tab === "walls"
      ? [wallsSection(project, ctx, events)]
      : [
          shedSection(project, building, events),
          framingSection(project, events),
          roofSection(project, events),
          floorSection(project, events),
          porchSection(project, events),
          sheathingSection(project, events),
          addParts(project, events),
        ].filter((s) => s !== null)),
  );
}

// Which of the building's sections are unfolded, remembered between sessions.
const OPEN_KEY = "sticks.panel.open";
const openSections = new Set<string>(["shed"]);
try {
  const saved = JSON.parse(localStorage.getItem(OPEN_KEY) ?? "null");
  if (Array.isArray(saved)) {
    openSections.clear();
    for (const key of saved) openSections.add(String(key));
  }
} catch {
  // Nothing remembered; the shed's size starts open.
}

/** Unfolds a section the next time the panel is drawn. */
export function openSection(key: string): void {
  openSections.add(key);
  rememberOpen();
}

function rememberOpen(): void {
  try {
    localStorage.setItem(OPEN_KEY, JSON.stringify([...openSections]));
  } catch {
    // Not remembered this time; folding still works.
  }
}

const CHEVRON = `<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M6 4l4 4-4 4"/></svg>`;

/** A button that folds `body` away, showing `summary` in its place. */
function foldButton(
  title: Node | string,
  summary: () => string,
  body: HTMLElement,
  isOpen: boolean,
  onToggle: (open: boolean) => void,
): HTMLButtonElement {
  const sum = h("span", { class: "fold-summary" });
  const chevron = h("span", { class: "chevron" });
  chevron.innerHTML = CHEVRON;
  const button = h("button", { type: "button", class: "fold-head" }, chevron, title, sum);
  const show = (open: boolean) => {
    body.hidden = !open;
    button.setAttribute("aria-expanded", String(open));
    // The summary is read when folding, so it shows the values just typed.
    sum.textContent = open ? "" : summary();
  };
  button.addEventListener("click", () => {
    const open = button.getAttribute("aria-expanded") !== "true";
    show(open);
    onToggle(open);
  });
  show(isOpen);
  return button;
}

/** One of the building's parts, folding to a line of its values. */
function foldSection(
  key: string,
  title: string,
  summary: () => string,
  body: (Node | null)[],
  remove?: [label: string, remove: () => void],
): HTMLElement {
  const content = h(
    "div",
    { class: "fold-body", id: `fold-${key}` },
    ...body,
    remove ? h("button", { type: "button", class: "remove-button", onclick: remove[1] }, remove[0]) : null,
  );
  const head = foldButton(h("span", { class: "fold-title" }, title), summary, content, openSections.has(key), (open) => {
    if (open) openSections.add(key);
    else openSections.delete(key);
    rememberOpen();
  });
  head.setAttribute("aria-controls", content.id);
  return h("section", { class: "panel-section fold", "data-section": key }, h("h2", {}, head), content);
}

/** Buttons for the parts the building doesn't have yet. */
function addParts(project: ProjectJson, events: PanelEvents): HTMLElement | null {
  const add = (key: string, text: string, title: string, make: () => void) =>
    h(
      "button",
      {
        type: "button",
        class: "add-button",
        title,
        onclick: () => {
          make();
          openSection(key);
          events.restructured();
        },
      },
      text,
    );
  const buttons = [
    project.roof
      ? null
      : add("roof", "+ Roof", "A lean-to roof at 2:12", () => {
          project.roof = { kind: "lean_to", pitch: "2:12", rafter: "2x6", front_overhang: '12"', back_overhang: '12"' };
        }),
    project.floor
      ? null
      : add("floor", "+ Floor", "A floor on beams; without one the walls stand on a slab", () => {
          project.floor = {
            joist: "2x6",
            beam: "4x6",
            beam_plies: 1,
            beams: 2,
            support: "piers",
            pier_spacing: "6'",
            pier_inset: '12"',
            subfloor: '3/4"',
          };
        }),
    project.porch
      ? null
      : add("porch", "+ Porch", "A porch off the front", () => {
          const p: PorchJson = { depth: "6'", post: "4x4", beam: "2x8", beam_plies: 2 };
          // A deck hangs from the floor's rim, so it needs a floor.
          if (project.floor) p.deck = { beam: "4x6", decking: "5/4x6", step_down: '1"' };
          project.porch = p;
        }),
    project.sheathing
      ? null
      : add("sheathing", "+ Sheathing", "Sheathing on the walls and roof", () => {
          project.sheathing = { wall: '7/16"', roof: '1/2"' };
        }),
  ].filter((b) => b !== null);
  if (buttons.length === 0) return null;
  return h(
    "section",
    { class: "panel-section add-parts" },
    h("p", { class: "section-hint" }, "Add to the shed"),
    h("div", { class: "add-row" }, ...buttons),
  );
}

/** Stud spacing, wall lumber and the layout end: rules every wall shares. */
function rulesFields(project: ProjectJson, events: PanelEvents): HTMLElement[] {
  const rules = project.rules;
  const oc = parseLength(rules.on_center);
  return [
    h(
      "div",
      { class: "row" },
      segmented(
        "Stud spacing",
        [
          ['16"', '16"'],
          ['24"', '24"'],
        ],
        () => ("ok" in oc && oc.ok === 24 * 16 ? '24"' : '16"'),
        (v) => (rules.on_center = v),
        events.changed,
      ),
      select("Wall lumber", ["2x4", "2x6"], () => rules.wall_lumber, (v) => (rules.wall_lumber = v), events),
    ),
    segmented(
      "Measure layout from",
      [
        ["left", "Left end"],
        ["right", "Right end"],
      ],
      () => rules.layout_from,
      (v) => (rules.layout_from = v),
      events.changed,
    ),
  ];
}


function shedSection(project: ProjectJson, b: BuildingJson, events: PanelEvents): HTMLElement {
  return foldSection("shed", "Shed", () => `${b.width} × ${b.depth} · ${b.height} front`, [
    h(
      "div",
      { class: "row" },
      lengthField("Width", () => b.width, (v) => (b.width = v), events, "Along the front"),
      lengthField("Depth", () => b.depth, (v) => (b.depth = v), events, "Front to back"),
    ),
    lengthField(
      "Front wall height",
      () => b.height,
      (v) => (b.height = v),
      events,
      project.roof && project.roof.kind !== "gable"
        ? "Floor to top of cap plate. The other walls follow the roof."
        : "Floor to top of cap plate",
    ),
  ]);
}

function framingSection(project: ProjectJson, events: PanelEvents): HTMLElement {
  const rules = project.rules;
  return foldSection(
    "framing",
    "Framing",
    () => {
      const oc = parseLength(rules.on_center);
      const spacing = "ok" in oc ? `${oc.ok / 16}"` : rules.on_center;
      return `${rules.wall_lumber} at ${spacing} · from ${rules.layout_from}`;
    },
    rulesFields(project, events),
  );
}

function roofSection(project: ProjectJson, events: PanelEvents): HTMLElement | null {
  const roof = project.roof;
  if (!roof) return null;
  return foldSection(
    "roof",
    "Roof",
    () => `${roof.kind === "gable" ? "Gable" : "Lean-to"} · ${roof.pitch} · ${roof.rafter} rafters`,
    [
      segmented(
        "Shape",
        [
          ["lean_to", "Lean-to"],
          ["gable", "Gable"],
        ],
        () => roof.kind ?? "lean_to",
        (v) => (roof.kind = v),
        events.restructured,
      ),
      h(
        "div",
        { class: "row" },
        pitchField(roof, events),
        select("Rafters", RAFTERS, () => roof.rafter, (v) => (roof.rafter = v), events),
      ),
      h(
        "div",
        { class: "row" },
        lengthField("Front overhang", () => roof.front_overhang, (v) => (roof.front_overhang = v), events),
        lengthField("Back overhang", () => roof.back_overhang, (v) => (roof.back_overhang = v), events),
      ),
    ],
    [
      "Remove roof",
      () => {
        delete project.roof;
        events.restructured();
      },
    ],
  );
}

/** Rise per foot, like `3:12` or `3-1/2:12`. Shows the back wall height it gives. */
function pitchField(roof: RoofJson, events: PanelEvents): HTMLElement {
  const message = h("div", { class: "field-error", role: "alert" });
  const input = h("input", {
    type: "text",
    value: roof.pitch,
    spellcheck: "false",
    autocomplete: "off",
    "aria-label": "Pitch",
    oninput: () => {
      const [rise, run] = input.value.split(":");
      const ok = run?.trim() === "12" && "ok" in parseLength(rise);
      input.classList.toggle("invalid", !ok);
      message.textContent = ok ? "" : "Try a pitch like 3:12";
      if (!ok) return;
      roof.pitch = input.value.trim();
      events.changed();
    },
  });
  return h(
    "label",
    { class: "field" },
    h("span", { class: "field-label" }, "Pitch"),
    input,
    h("span", { class: "field-hint", "data-live": "back-height" }),
    message,
  );
}


function floorSection(project: ProjectJson, events: PanelEvents): HTMLElement | null {
  const floor = project.floor;
  if (!floor) return null;
  return foldSection(
    "floor",
    "Floor",
    () => `${floor.joist} joists · ${floor.beams} beams on ${floor.support === "piers" ? "piers" : "grade"}`,
    [
      h(
        "div",
        { class: "row" },
        select("Joists", JOISTS, () => floor.joist, (v) => (floor.joist = v), events),
        select("Beams", BEAMS, () => floor.beam, (v) => (floor.beam = v), events),
      ),
      h(
        "div",
        { class: "row" },
        stepper("Beam lines", () => floor.beams, (v) => (floor.beams = v), 2, 6, events),
        segmented(
          "Beams sit on",
          [
            ["piers", "Piers"],
            ["skids", "Grade"],
          ],
          () => floor.support,
          (v) => (floor.support = v),
          events.restructured,
        ),
      ),
      floor.support === "piers"
        ? lengthField("Most room between piers", () => floor.pier_spacing, (v) => (floor.pier_spacing = v), events)
        : null,
    ],
    [
      "Remove floor",
      () => {
        delete project.floor;
        events.restructured();
      },
    ],
  );
}

function porchSection(project: ProjectJson, events: PanelEvents): HTMLElement | null {
  const porch = project.porch;
  if (!porch) return null;
  const deck = porch.deck;
  return foldSection(
    "porch",
    "Porch",
    () => `${porch.depth ?? "6'"} deep · ${porch.deck ? "deck" : "open ground"}`,
    [
      h(
        "div",
        { class: "row" },
        lengthField("Depth", () => porch.depth ?? "6'", (v) => (porch.depth = v), events, "Out from the front wall"),
        lengthField("Rafter overhang", () => porch.overhang ?? '12"', (v) => (porch.overhang = v), events, "Past the beam"),
      ),
      h(
        "div",
        { class: "row" },
        select("Posts", POSTS, () => porch.post ?? "4x4", (v) => (porch.post = v), events),
        lengthField("Most room between posts", () => porch.post_spacing ?? "8'", (v) => (porch.post_spacing = v), events),
      ),
      h(
        "div",
        { class: "row" },
        select("Beam", PORCH_BEAMS, () => porch.beam ?? "2x8", (v) => (porch.beam = v), events),
        stepper("Beam plies", () => porch.beam_plies ?? 2, (v) => (porch.beam_plies = v), 1, 3, events),
      ),
      segmented(
        "Under the porch",
        [
          ["deck", "Deck"],
          ["ground", "Open ground"],
        ],
        () => (deck ? "deck" : "ground"),
        (v) => {
          if (v === "deck") porch.deck = { beam: "4x6", decking: "5/4x6", step_down: '1"' };
          else delete porch.deck;
        },
        events.restructured,
      ),
      deck
        ? h(
            "div",
            { class: "row" },
            select("Decking", DECKING, () => deck.decking ?? "5/4x6", (v) => (deck.decking = v), events),
            lengthField("Step down", () => deck.step_down ?? '1"', (v) => (deck.step_down = v), events, "Below the shed floor"),
          )
        : null,
    ],
    [
      "Remove porch",
      () => {
        delete project.porch;
        events.restructured();
      },
    ],
  );
}

function sheathingSection(project: ProjectJson, events: PanelEvents): HTMLElement | null {
  const sheathing = project.sheathing;
  if (!sheathing) return null;
  const thickness = (part: "wall" | "roof", label: string) =>
    select(
      label,
      [...SHEATHING, NO_SHEATHING],
      () => sheathing[part] ?? NO_SHEATHING,
      (v) => {
        if (v === NO_SHEATHING) delete sheathing[part];
        else sheathing[part] = v;
      },
      events,
    );
  const summary = () =>
    [sheathing.wall ? `${sheathing.wall} walls` : null, sheathing.roof ? `${sheathing.roof} roof` : null]
      .filter((p) => p !== null)
      .join(" · ") || "None";
  return foldSection(
    "sheathing",
    "Sheathing",
    summary,
    [
      h("div", { class: "row" }, thickness("wall", "Walls"), thickness("roof", "Roof")),
      h("p", { class: "section-hint" }, "4 × 8 sheets, joints on framing, cut around doors and windows."),
    ],
    [
      "Remove sheathing",
      () => {
        delete project.sheathing;
        events.restructured();
      },
    ],
  );
}

function stepper(
  label: string,
  get: () => number,
  set: (v: number) => void,
  min: number,
  max: number,
  events: PanelEvents,
): HTMLElement {
  const value = h("output", { "aria-live": "polite" }, String(get()));
  const step = (by: number) => () => {
    const next = Math.min(max, Math.max(min, get() + by));
    if (next === get()) return;
    set(next);
    value.textContent = String(next);
    less.disabled = next <= min;
    more.disabled = next >= max;
    events.changed();
  };
  const less = h("button", { type: "button", "aria-label": `Fewer ${label.toLowerCase()}`, onclick: step(-1), disabled: get() <= min }, "−");
  const more = h("button", { type: "button", "aria-label": `More ${label.toLowerCase()}`, onclick: step(1), disabled: get() >= max }, "+");
  return h(
    "div",
    { class: "field" },
    h("span", { class: "field-label" }, label),
    h("div", { class: "stepper", role: "group", "aria-label": label }, less, value, more),
  );
}

function wallsSection(project: ProjectJson, ctx: PanelContext, events: PanelEvents): HTMLElement {
  const id = ctx.wall;
  let wall = project.wall.find((w) => w.id === id);
  const side = project.building !== undefined && isSide(id);
  // A building's walls are listed in the file only once something is on them.
  const ensure = (): WallJson => {
    if (!wall) {
      wall = { id, tees: [], opening: [] };
      project.wall.push(wall);
    }
    return wall;
  };
  const picker =
    ctx.wallIds.length > 1
      ? segmented(
          project.building ? "Wall, seen from outside" : "Wall",
          ctx.wallIds.map((w) => [w, wallName(w)] as [string, string]),
          () => id,
          (v) => ctx.selectWall(v),
          () => {},
        )
      : null;
  const sizeFields =
    !side && wall
      ? [
          h(
            "div",
            { class: "row" },
            lengthField("Length", () => wall!.length ?? "", (v) => (wall!.length = v), events),
            lengthField("Height", () => wall!.height ?? "", (v) => (wall!.height = v), events, "Floor to top of cap plate"),
          ),
          h(
            "div",
            { class: "row" },
            endField("Left end", wall, "left_end", events),
            endField("Right end", wall, "right_end", events),
          ),
        ]
      : [];
  return h(
    "div",
    { class: "walls" },
    h(
      "section",
      { class: "panel-section" },
      // A building's tab already names the walls.
      project.building ? null : h("h2", {}, "Wall"),
      picker,
      ...sizeFields,
      ...(project.building ? [] : rulesFields(project, events)),
    ),
    openingsSection(wall, ensure, ctx.wallLength(id), events),
    teesSection(wall, ensure, ctx.wallLength(id), events),
  );
}

function endField(
  label: string,
  wall: WallJson,
  key: "left_end" | "right_end",
  events: PanelEvents,
): HTMLElement {
  return segmented(
    label,
    [
      ["plain", "Plain"],
      ["corner", "Corner"],
    ],
    () => wall[key] ?? "plain",
    (v) => (wall[key] = v),
    events.changed,
  );
}

/** Openings folded to their summary; new ones start unfolded. */
const foldedOpenings = new WeakSet<object>();

function openingsSection(
  wall: WallJson | undefined,
  ensure: () => WallJson,
  length16: number | null,
  events: PanelEvents,
): HTMLElement {
  const add = (kind: OpeningKind) => () => {
    const w = ensure();
    w.opening.push(newOpening(kind, freeSpot(w, length16)));
    events.restructured();
  };
  const openings = wall?.opening ?? [];
  const cards = openings.map((o, i) => {
    const name = `${KIND_NAMES[o.kind]} ${i + 1}`;
    const body = h(
      "div",
      { class: "card-body" },
      h(
        "div",
        { class: "row" },
        lengthField("From left end", () => o.offset, (v) => (o.offset = v), events),
        lengthField("Width", () => o.width, (v) => (o.width = v), events),
      ),
      h(
        "div",
        { class: "row" },
        lengthField("Height", () => o.height, (v) => (o.height = v), events),
        o.kind === "door"
          ? select("Header", HEADERS, () => o.header, (v) => (o.header = v), events)
          : lengthField("Sill height", () => o.sill_height, (v) => (o.sill_height = v), events),
      ),
      o.kind === "door" ? null : select("Header", HEADERS, () => o.header, (v) => (o.header = v), events),
      h(
        "button",
        {
          type: "button",
          class: "remove-button",
          "aria-label": `Remove ${name.toLowerCase()}`,
          onclick: () => {
            openings.splice(i, 1);
            events.restructured();
          },
        },
        "Remove",
      ),
    );
    const head = foldButton(
      h("span", { class: "card-title" }, name),
      () => `${o.width} × ${o.height} at ${o.offset}`,
      body,
      !foldedOpenings.has(o),
      (open) => (open ? foldedOpenings.delete(o) : foldedOpenings.add(o)),
    );
    return h("div", { class: "card" }, head, body);
  });
  return h(
    "section",
    { class: "panel-section", "data-section": "openings" },
    h("h2", {}, "Openings"),
    ...cards,
    openings.length === 0 ? h("p", { class: "empty" }, "No doors or windows in this wall.") : null,
    h(
      "div",
      { class: "add-row" },
      h("button", { type: "button", class: "add-button", onclick: add("door") }, "+ Door"),
      h("button", { type: "button", class: "add-button", onclick: add("window") }, "+ Window"),
      h("button", { type: "button", class: "add-button", onclick: add("penetration") }, "+ Penetration"),
    ),
  );
}

function teesSection(
  wall: WallJson | undefined,
  ensure: () => WallJson,
  length16: number | null,
  events: PanelEvents,
): HTMLElement {
  const tees = wall?.tees ?? [];
  const rows = tees.map((_, i) =>
    h(
      "div",
      { class: "tee-row" },
      lengthField(
        "Partition center",
        () => tees[i],
        (v) => (tees[i] = v),
        events,
      ),
      h(
        "button",
        {
          type: "button",
          class: "icon-button",
          "aria-label": `Remove partition ${i + 1}`,
          title: "Remove",
          onclick: () => {
            tees.splice(i, 1);
            events.restructured();
          },
        },
        "×",
      ),
    ),
  );
  return h(
    "section",
    { class: "panel-section" },
    h("h2", {}, "Partitions"),
    h("p", { class: "section-hint" }, "Interior walls that tee in get a stud on each side."),
    ...rows,
    h(
      "div",
      { class: "add-row" },
      h(
        "button",
        {
          type: "button",
          class: "add-button",
          onclick: () => {
            ensure().tees.push(length16 === null ? '48"' : `${Math.round(length16 / 32)}"`);
            events.restructured();
          },
        },
        "+ Partition",
      ),
    ),
  );
}

/** Puts a new opening a few feet past the last one, or near the left end. */
function freeSpot(wall: WallJson, length16: number | null): string {
  let end = 24 * 16;
  for (const o of wall.opening) {
    const offset = parseLength(o.offset);
    const width = parseLength(o.width);
    if ("ok" in offset && "ok" in width) end = Math.max(end, offset.ok + width.ok + 24 * 16);
  }
  if (length16 !== null && end > length16 - 60 * 16) end = 24 * 16;
  return `${Math.round(end / 16)}"`;
}
