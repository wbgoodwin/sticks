// The parameter panel: every field edits the project in place and asks for a
// re-frame. The panel is rebuilt only when its structure changes (an opening
// added or removed), so typing never loses focus.

import { parseLength, type OpeningKind, type ProjectJson, type StockJson, type WallJson } from "./core";
import { newOpening } from "./state";

export interface PanelEvents {
  /** A value changed; re-frame. */
  changed(): void;
  /** Openings or tees were added or removed; rebuild the panel too. */
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
    ...options.map((o) => h("option", { value: o, selected: o === get() }, o)),
  );
  return h("label", { class: "field" }, h("span", { class: "field-label" }, label), el);
}

const KIND_NAMES: Record<OpeningKind, string> = {
  door: "Door",
  window: "Window",
  penetration: "Penetration",
};

const HEADERS = ["2x4", "2x6", "2x8", "2x10", "2x12"];

export function renderPanel(root: HTMLElement, project: ProjectJson, events: PanelEvents): void {
  const wall = project.wall[0];
  const rules = project.rules;
  root.replaceChildren(
    h(
      "section",
      { class: "panel-section" },
      h("h2", {}, "Wall"),
      h(
        "div",
        { class: "row" },
        lengthField("Length", () => wall.length, (v) => (wall.length = v), events),
        lengthField("Height", () => wall.height, (v) => (wall.height = v), events, "Floor to top of cap plate"),
      ),
      h(
        "div",
        { class: "row" },
        segmented(
          "Stud spacing",
          [
            ['16"', '16"'],
            ['24"', '24"'],
          ],
          () => (rules.on_center === '24"' ? '24"' : '16"'),
          (v) => (rules.on_center = v),
          events.changed,
        ),
        select("Lumber", ["2x4", "2x6"], () => rules.wall_lumber, (v) => (rules.wall_lumber = v), events),
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
      h(
        "div",
        { class: "row" },
        endField("Left end", wall, "left_end", events),
        endField("Right end", wall, "right_end", events),
      ),
    ),
    openingsSection(wall, events),
    teesSection(wall, events),
    lumberSection(project, events),
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
    () => wall[key],
    (v) => (wall[key] = v),
    events.changed,
  );
}

function openingsSection(wall: WallJson, events: PanelEvents): HTMLElement {
  const add = (kind: OpeningKind) => () => {
    wall.opening.push(newOpening(kind, freeSpot(wall)));
    events.restructured();
  };
  const cards = wall.opening.map((o, i) =>
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
              wall.opening.splice(i, 1);
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
    wall.opening.length === 0 ? h("p", { class: "empty" }, "No doors or windows yet.") : null,
    h(
      "div",
      { class: "add-row" },
      h("button", { type: "button", class: "add-button", onclick: add("door") }, "+ Door"),
      h("button", { type: "button", class: "add-button", onclick: add("window") }, "+ Window"),
      h("button", { type: "button", class: "add-button", onclick: add("penetration") }, "+ Penetration"),
    ),
  );
}

function teesSection(wall: WallJson, events: PanelEvents): HTMLElement {
  const rows = wall.tees.map((_, i) =>
    h(
      "div",
      { class: "tee-row" },
      lengthField(
        "Partition center",
        () => wall.tees[i],
        (v) => (wall.tees[i] = v),
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
            wall.tees.splice(i, 1);
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
            wall.tees.push(midpoint(wall));
            events.restructured();
          },
        },
        "+ Partition",
      ),
    ),
  );
}

/** Puts a new opening a few feet past the last one, or near the left end. */
function freeSpot(wall: WallJson): string {
  let end = 24 * 16;
  for (const o of wall.opening) {
    const offset = parseLength(o.offset);
    const width = parseLength(o.width);
    if ("ok" in offset && "ok" in width) end = Math.max(end, offset.ok + width.ok + 24 * 16);
  }
  const len = parseLength(wall.length);
  if ("ok" in len && end > len.ok - 60 * 16) end = 24 * 16;
  return `${Math.round(end / 16)}"`;
}

function midpoint(wall: WallJson): string {
  const len = parseLength(wall.length);
  return "ok" in len ? `${Math.round(len.ok / 32)}"` : "48\"";
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
