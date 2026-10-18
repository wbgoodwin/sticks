// The settings dialog: the lumber yard the cut list buys from, with the
// lengths and prices of every board size and sheet, and the saw's kerf. It all
// saves with the project, like the rest of the design.

import { parseLength, type ProjectJson, type SheetJson, type StockJson } from "./core";
import { h, lengthField, type PanelEvents } from "./panel";

/** What the right side shows: one board size, one sheet, or the kerf. */
type Pick = { kind: "stock"; size: string } | { kind: "sheet"; thickness: string } | { kind: "kerf" };

let pick: Pick | null = null;

/** Draws the dialog's body. `changed` re-frames after any edit. */
export function renderSettings(root: HTMLElement, project: ProjectJson, changed: () => void): void {
  const stock = project.stock ?? [];
  const sheets = project.sheet ?? [];
  // A size the design no longer uses can drop off the list.
  if (
    !pick ||
    (pick.kind === "stock" && !stock.some((s) => s.size === (pick as { size: string }).size)) ||
    (pick.kind === "sheet" && !sheets.some((s) => s.thickness === (pick as { thickness: string }).thickness))
  ) {
    pick = stock[0] ? { kind: "stock", size: stock[0].size } : { kind: "kerf" };
  }
  const nav = h("nav", { class: "settings-nav", "aria-label": "Lumber yard" });
  const drawNav = () => {
    const item = (p: Pick, label: string, status: string) =>
      h(
        "button",
        {
          type: "button",
          "aria-current": String(sameItem(p, pick!)),
          onclick: () => {
            pick = p;
            renderSettings(root, project, changed);
          },
        },
        h("span", {}, label),
        h("em", {}, status),
      );
    nav.replaceChildren(
      h("span", { class: "settings-group" }, "Boards"),
      ...stock.map((s) => item({ kind: "stock", size: s.size }, s.size, priced(s))),
      ...(sheets.length > 0 ? [h("span", { class: "settings-group" }, "Sheets")] : []),
      ...sheets.map((s) =>
        item(
          { kind: "sheet", thickness: s.thickness },
          `${s.thickness} ${sheetUses(project, s)}`.trim(),
          s.price_cents === undefined ? "no price" : `$${formatCents(s.price_cents)}`,
        ),
      ),
      h("span", { class: "settings-group" }, "Cutting"),
      item({ kind: "kerf" }, "Saw kerf", project.rules.kerf ?? '1/8"'),
    );
  };
  drawNav();
  // Typing keeps its focus: only the list's status line is redrawn.
  const events: PanelEvents = {
    changed() {
      changed();
      drawNav();
    },
    restructured() {
      changed();
      renderSettings(root, project, changed);
    },
  };
  const p = pick;
  let detail: HTMLElement;
  if (p.kind === "stock") detail = stockDetail(stock.find((s) => s.size === p.size)!, events);
  else if (p.kind === "sheet") detail = sheetDetail(project, sheets.find((s) => s.thickness === p.thickness)!, events);
  else {
    const rules = project.rules;
    detail = h(
      "div",
      { class: "settings-detail" },
      h("h3", {}, "Saw kerf"),
      lengthField("Kerf", () => rules.kerf ?? '1/8"', (v) => (rules.kerf = v), events, "Lost to the blade on every cut"),
    );
  }
  root.replaceChildren(nav, detail);
}

function sameItem(a: Pick, b: Pick): boolean {
  if (a.kind === "stock" && b.kind === "stock") return a.size === b.size;
  if (a.kind === "sheet" && b.kind === "sheet") return a.thickness === b.thickness;
  return a.kind === b.kind;
}

/** How much of a size's list has prices. */
function priced(stock: StockJson): string {
  const n = stock.lengths.filter((_, i) => (stock.prices_cents[i] ?? null) !== null).length;
  if (n === 0) return "no prices";
  if (n === stock.lengths.length) return "priced";
  return `${n} of ${stock.lengths.length} priced`;
}

function stockDetail(stock: StockJson, events: PanelEvents): HTMLElement {
  return h(
    "div",
    { class: "settings-detail" },
    h("h3", {}, stock.size),
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
    h(
      "p",
      { class: "settings-note" },
      "With a price on every length of every size, the cut list plans for the lowest cost. Otherwise it plans for the least lumber.",
    ),
  );
}

/** What a sheet thickness covers, such as `subfloor` or `wall and roof`. */
function sheetUses(project: ProjectJson, sheet: SheetJson): string {
  const uses = [
    project.floor?.subfloor === sheet.thickness ? "subfloor" : null,
    project.sheathing?.wall === sheet.thickness ? "wall" : null,
    project.sheathing?.roof === sheet.thickness ? "roof" : null,
  ].filter((u) => u !== null);
  return uses.join(" and ");
}

function sheetDetail(project: ProjectJson, sheet: SheetJson, events: PanelEvents): HTMLElement {
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
    { class: "settings-detail" },
    h("h3", {}, `${sheet.thickness} ${sheetUses(project, sheet)} sheets`.replace(/\s+/g, " ")),
    h(
      "div",
      { class: "stock-head", "aria-hidden": "true" },
      h("span", {}, "Size"),
      h("span", {}, "Price each"),
    ),
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
