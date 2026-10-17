// The parameter panel: every field edits the project in place and asks for a
// re-frame. The panel is rebuilt only when its structure changes (an opening
// added, the roof removed, another wall picked), so typing never loses focus.

import {
  isSide,
  parseLength,
  type BuildingJson,
  type OpeningKind,
  type PorchJson,
  type ProjectJson,
  type RoofJson,
  type SheetJson,
  type StockJson,
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

function h<K extends keyof HTMLElementTagNameMap>(
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
function lengthField(
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

/** What the panel needs to know about the framed project. */
export interface PanelContext {
  /** Every wall, in drawing order. */
  wallIds: string[];
  /** The wall whose openings are being edited. */
  wall: string;
  selectWall(id: string): void;
  /** A wall's framed length in sixteenths, when it frames. */
  wallLength(id: string): number | null;
}

export function renderPanel(
  root: HTMLElement,
  project: ProjectJson,
  ctx: PanelContext,
  events: PanelEvents,
): void {
  const building = project.building;
  root.replaceChildren(
    ...(building
      ? [
          shedSection(project, building, events),
          roofSection(project, events),
          floorSection(project, events),
          porchSection(project, events),
          sheathingSection(project, events),
          wallsSection(project, ctx, events),
        ]
      : [wallsSection(project, ctx, events)]),
    lumberSection(project, events),
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
  return h(
    "section",
    { class: "panel-section" },
    h("h2", {}, "Shed"),
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
    ...rulesFields(project, events),
  );
}

function roofSection(project: ProjectJson, events: PanelEvents): HTMLElement {
  const roof = project.roof;
  if (!roof) {
    return h(
      "section",
      { class: "panel-section" },
      h("h2", {}, "Roof"),
      h("p", { class: "empty" }, "No roof yet."),
      h(
        "div",
        { class: "add-row" },
        h(
          "button",
          {
            type: "button",
            class: "add-button",
            onclick: () => {
              project.roof = {
                kind: "lean_to",
                pitch: "2:12",
                rafter: "2x6",
                front_overhang: '12"',
                back_overhang: '12"',
              };
              events.restructured();
            },
          },
          "+ Roof",
        ),
      ),
    );
  }
  return h(
    "section",
    { class: "panel-section" },
    sectionHead("Roof", "Remove the roof", () => {
      delete project.roof;
      events.restructured();
    }),
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

function floorSection(project: ProjectJson, events: PanelEvents): HTMLElement {
  const floor = project.floor;
  if (!floor) {
    return h(
      "section",
      { class: "panel-section" },
      h("h2", {}, "Floor"),
      h("p", { class: "empty" }, "No floor yet; the walls stand on a slab."),
      h(
        "div",
        { class: "add-row" },
        h(
          "button",
          {
            type: "button",
            class: "add-button",
            onclick: () => {
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
              events.restructured();
            },
          },
          "+ Floor on beams",
        ),
      ),
    );
  }
  return h(
    "section",
    { class: "panel-section" },
    sectionHead("Floor", "Remove the floor", () => {
      delete project.floor;
      events.restructured();
    }),
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
  );
}

function porchSection(project: ProjectJson, events: PanelEvents): HTMLElement {
  const porch = project.porch;
  if (!porch) {
    return h(
      "section",
      { class: "panel-section" },
      h("h2", {}, "Porch"),
      h("p", { class: "empty" }, "No porch yet."),
      h(
        "div",
        { class: "add-row" },
        h(
          "button",
          {
            type: "button",
            class: "add-button",
            onclick: () => {
              const p: PorchJson = { depth: "6'", post: "4x4", beam: "2x8", beam_plies: 2 };
              // A deck hangs from the floor's rim, so it needs a floor.
              if (project.floor) p.deck = { beam: "4x6", decking: "5/4x6", step_down: '1"' };
              project.porch = p;
              events.restructured();
            },
          },
          "+ Porch off the front",
        ),
      ),
    );
  }
  const deck = porch.deck;
  return h(
    "section",
    { class: "panel-section" },
    sectionHead("Porch", "Remove the porch", () => {
      delete project.porch;
      events.restructured();
    }),
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
  );
}

function sheathingSection(project: ProjectJson, events: PanelEvents): HTMLElement {
  const sheathing = project.sheathing;
  if (!sheathing) {
    return h(
      "section",
      { class: "panel-section" },
      h("h2", {}, "Sheathing"),
      h("p", { class: "empty" }, "No sheathing yet."),
      h(
        "div",
        { class: "add-row" },
        h(
          "button",
          {
            type: "button",
            class: "add-button",
            onclick: () => {
              project.sheathing = { wall: '7/16"', roof: '1/2"' };
              events.restructured();
            },
          },
          "+ Walls and roof",
        ),
      ),
    );
  }
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
  return h(
    "section",
    { class: "panel-section" },
    sectionHead("Sheathing", "Remove the sheathing", () => {
      delete project.sheathing;
      events.restructured();
    }),
    h("div", { class: "row" }, thickness("wall", "Walls"), thickness("roof", "Roof")),
    h("p", { class: "section-hint" }, "4 × 8 sheets, joints on framing, cut around doors and windows."),
  );
}

function sectionHead(title: string, removeLabel: string, remove: () => void): HTMLElement {
  return h(
    "div",
    { class: "section-head" },
    h("h2", {}, title),
    h("button", { type: "button", class: "icon-button", "aria-label": removeLabel, title: removeLabel, onclick: remove }, "×"),
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
      h("h2", {}, project.building ? "Walls" : "Wall"),
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
  const cards = openings.map((o, i) =>
    h(
      "div",
      { class: "card" },
      h(
        "div",
        { class: "card-head" },
        h("span", { class: "card-title" }, `${KIND_NAMES[o.kind]} ${i + 1}`),
        h(
          "button",
          {
            type: "button",
            class: "icon-button",
            "aria-label": `Remove ${KIND_NAMES[o.kind].toLowerCase()} ${i + 1}`,
            title: "Remove",
            onclick: () => {
              openings.splice(i, 1);
              events.restructured();
            },
          },
          "×",
        ),
      ),
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
    ),
  );
  return h(
    "section",
    { class: "panel-section" },
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

function lumberSection(project: ProjectJson, events: PanelEvents): HTMLElement {
  const rules = project.rules;
  const cards = (project.stock ?? []).map((stock) =>
    h(
      "div",
      { class: "card" },
      h("div", { class: "card-head" }, h("span", { class: "card-title" }, stock.size)),
      h(
        "div",
        { class: "stock-head", "aria-hidden": "true" },
        h("span", {}, "Length"),
        h("span", {}, "Price each"),
      ),
      ...stock.lengths.map((_, i) => stockRow(stock, i, events)),
      h(
        "div",
        { class: "add-row" },
        h(
          "button",
          {
            type: "button",
            class: "add-button",
            onclick: () => {
              stock.lengths.push(nextLength(stock));
              if (stock.prices_cents.length > 0) stock.prices_cents.push(null);
              events.restructured();
            },
          },
          "+ Length",
        ),
      ),
    ),
  );
  return h(
    "section",
    { class: "panel-section" },
    h("h2", {}, "Lumber yard"),
    h(
      "p",
      { class: "section-hint" },
      "Boards the cut list can buy. With a price on every length it plans for the lowest cost; otherwise for the least lumber.",
    ),
    lengthField("Saw kerf", () => rules.kerf ?? '1/8"', (v) => (rules.kerf = v), events, "Lost to the blade on every cut"),
    ...cards,
    ...(project.sheet ?? []).map((sheet) => sheetCard(sheet, sheetUses(project, sheet), events)),
  );
}

/** What a sheet thickness covers, such as `subfloor` or `wall and roof`. */
function sheetUses(project: ProjectJson, sheet: SheetJson): string {
  const uses = [
    project.floor?.subfloor === sheet.thickness ? "subfloor" : null,
    project.sheathing?.wall === sheet.thickness ? "wall" : null,
    project.sheathing?.roof === sheet.thickness ? "roof" : null,
  ].filter((u) => u !== null);
  if (uses.length === 0) return "sheets";
  return `${uses.join(" and ")} sheets`;
}

function sheetCard(sheet: SheetJson, uses: string, events: PanelEvents): HTMLElement {
  const message = h("div", { class: "field-error", role: "alert" });
  const price = h("input", {
    type: "text",
    inputmode: "decimal",
    placeholder: "$",
    value: formatCents(sheet.price_cents ?? null),
    autocomplete: "off",
    "aria-label": `${sheet.thickness} sheet price`,
    oninput: () => {
      const cents = parseCents(price.value);
      if (cents === undefined) {
        price.classList.add("invalid");
        message.textContent = "Try a price like 45.99";
        return;
      }
      price.classList.remove("invalid");
      message.textContent = "";
      if (cents === null) delete sheet.price_cents;
      else sheet.price_cents = cents;
      events.changed();
    },
  });
  return h(
    "div",
    { class: "card" },
    h("div", { class: "card-head" }, h("span", { class: "card-title" }, `${sheet.thickness} ${uses}`)),
    h(
      "div",
      { class: "stock-row" },
      h("div", { class: "sheet-size" }, `${sheet.width ?? "4'"} × ${sheet.length ?? "8'"}`),
      h("label", { class: "field" }, price, message),
    ),
  );
}

function stockRow(stock: StockJson, i: number, events: PanelEvents): HTMLElement {
  const message = h("div", { class: "field-error", role: "alert" });
  const price = h("input", {
    type: "text",
    inputmode: "decimal",
    placeholder: "$",
    value: formatCents(stock.prices_cents[i] ?? null),
    autocomplete: "off",
    "aria-label": `${stock.size} ${stock.lengths[i]} price`,
    oninput: () => {
      const cents = parseCents(price.value);
      if (cents === undefined) {
        price.classList.add("invalid");
        message.textContent = "Try a price like 4.98";
        return;
      }
      price.classList.remove("invalid");
      message.textContent = "";
      while (stock.prices_cents.length < stock.lengths.length) stock.prices_cents.push(null);
      stock.prices_cents[i] = cents;
      if (stock.prices_cents.every((p) => p === null)) stock.prices_cents = [];
      events.changed();
    },
  });
  const length = lengthField(`${stock.size} length`, () => stock.lengths[i], (v) => (stock.lengths[i] = v), events);
  // The card's column heads label these rows.
  length.querySelector(".field-label")?.remove();
  return h(
    "div",
    { class: "stock-row" },
    length,
    h("label", { class: "field" }, price, message),
    h(
      "button",
      {
        type: "button",
        class: "icon-button",
        "aria-label": `Remove ${stock.size} ${stock.lengths[i]}`,
        title: "Remove",
        disabled: stock.lengths.length === 1,
        onclick: () => {
          stock.lengths.splice(i, 1);
          stock.prices_cents.splice(i, 1);
          events.restructured();
        },
      },
      "×",
    ),
  );
}

/** Cents from `4.98` or `$4.98`; null when blank; undefined when unreadable. */
function parseCents(text: string): number | null | undefined {
  const t = text.trim().replace(/^\$\s*/, "");
  if (t === "") return null;
  const m = /^(\d+)(?:\.(\d{0,2}))?$/.exec(t);
  if (!m) return undefined;
  return Number(m[1]) * 100 + Number((m[2] ?? "").padEnd(2, "0"));
}

function formatCents(cents: number | null): string {
  return cents === null ? "" : (cents / 100).toFixed(2);
}

/** Two feet past the longest length on the list. */
function nextLength(stock: StockJson): string {
  const longest = Math.max(
    0,
    ...stock.lengths.map(parseLength).map((r) => ("ok" in r ? r.ok : 0)),
  );
  return `${Math.round(longest / 192) + 2}'`;
}
