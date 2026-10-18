// The shopping list and the board-by-board cut list, drawn from the core's
// plan. Boards cut the same way are shown once with a count.

import type { BoardView, CutListView, HardwareView } from "./core";
import { KIND_NAMES } from "./elevation";

function esc(text: string): string {
  return text.replace(/[&<>"]/g, (c) => `&${{ "&": "amp", "<": "lt", ">": "gt", '"': "quot" }[c]};`);
}

export function dollars(cents: number): string {
  return `$${(cents / 100).toFixed(2)}`;
}

/** Called with a section's key when the builder marks it had or not. */
export type OnHandToggle = (key: string) => void;

export function renderShopping(root: HTMLElement, plan: CutListView, toggle: OnHandToggle): void {
  const priced = plan.summary.total_cents !== null;
  const rows = plan.bom
    .map(
      (l) =>
        `<tr><td>${l.count}</td><td>${l.size}</td><td class="num">${l.length}</td>` +
        `<td class="num">${l.unit_price_cents === null ? "" : dollars(l.unit_price_cents)}</td>` +
        `<td class="num">${l.total_cents === null ? "" : dollars(l.total_cents)}</td></tr>`,
    )
    .join("");
  const total = priced
    ? `<tr class="total"><td></td><td>Total</td><td></td><td></td><td class="num">${dollars(plan.summary.total_cents!)}</td></tr>`
    : "";
  root.innerHTML = `
    ${have(plan)}
    <table>
      <thead><tr><th>Qty</th><th>Size</th><th class="num">Length</th><th class="num">Each</th><th class="num">Total</th></tr></thead>
      <tbody>${rows}${total}</tbody>
    </table>
    ${leftOff(plan)}
    ${notes(plan, priced ? [] : ["Add a price to every stock length in the lumber yard to see what this costs."])}
    ${hardware(plan.hardware)}
  `;
  for (const b of root.querySelectorAll<HTMLButtonElement>(".have button")) {
    b.addEventListener("click", () => toggle(b.dataset.key!));
  }
}

/** Connectors, then boxes of nails or screws, then what nothing fits. */
function hardware(hw: HardwareView): string {
  if (hw.parts.length === 0 && hw.warnings.length === 0) return "";
  const price = (c: number | null) => (c === null ? "" : dollars(c));
  const parts = hw.parts
    .map((p) => {
      const note = p.note ? `<span class="hw-note">${esc(p.note)}</span>` : "";
      return (
        `<tr><td>${p.count}</td><td><strong>${esc(p.model)}</strong> <span class="hw-name">${esc(p.name)}</span></td>` +
        `<td>${esc(list(p.joints))}${note}</td><td class="num">${price(p.unit_price_cents)}</td><td class="num">${price(p.total_cents)}</td></tr>`
      );
    })
    .join("");
  const fasteners = hw.fasteners
    .map(
      (f) =>
        `<tr><td>${f.boxes}</td><td>${esc(f.name)}</td><td>${f.count} pieces</td>` +
        `<td class="num">${price(f.box_price_cents)}</td><td class="num">${price(f.total_cents)}</td></tr>`,
    )
    .join("");
  const total =
    hw.total_cents === null
      ? ""
      : `<tr class="total"><td></td><td>Total</td><td></td><td></td><td class="num">${dollars(hw.total_cents)}</td></tr>`;
  const items = hw.warnings.map(esc);
  if (hw.parts.length > 0 && hw.total_cents === null) items.push("Add hardware prices in Settings to see what it costs.");
  const warnings = items.length === 0 ? "" : `<ul class="warnings">${items.map((t) => `<li>${t}</li>`).join("")}</ul>`;
  return `
    <h3 class="hardware-heading">Hardware <span>${esc(hw.brand)}</span></h3>
    <table class="hardware">
      <thead><tr><th>Qty</th><th>Part</th><th>For</th><th class="num">Each</th><th class="num">Total</th></tr></thead>
      <tbody>${parts}${fasteners}${total}</tbody>
    </table>
    ${warnings}
    <p class="hw-notice">Each joint gets the usual part for its lumber sizes. Sticks doesn't check loads, so check them against your local code.</p>
  `;
}

/** A button per stage of the build; pressed ones are already on hand. */
function have(plan: CutListView): string {
  if (plan.sections.length === 0) return "";
  const buttons = plan.sections
    .map(
      (s) =>
        `<button type="button" data-key="${esc(s.key)}" aria-pressed="${s.on_hand}" ` +
        `title="${s.on_hand ? "On hand, so it's left off. Click to buy it again." : "Click if you already have it, to leave it off."}">${esc(s.name)}</button>`,
    )
    .join("");
  return `<div class="have"><span class="have-label">Already have</span>${buttons}</div>`;
}

/** What the list leaves out, so its total doesn't look short. */
function leftOff(plan: CutListView): string {
  const names = plan.sections.filter((s) => s.on_hand).map((s) => s.name);
  if (names.length === 0) return "";
  const o = plan.on_hand;
  const parts = [o.boards > 0 ? plural(o.boards, "board") : "", o.sheets > 0 ? plural(o.sheets, "sheet") : ""].filter(Boolean);
  const cost = o.total_cents === null || o.total_cents === 0 ? "" : `, about ${dollars(o.total_cents)}`;
  const what = parts.length > 0 ? ` (${parts.join(" and ")}${cost})` : "";
  return `<p class="left-off">Leaves out ${esc(list(names).toLowerCase())}${what}, since you have ${names.length === 1 ? "it" : "them"}.</p>`;
}

function plural(n: number, word: string): string {
  return `${n} ${word}${n === 1 ? "" : "s"}`;
}

function list(items: string[]): string {
  if (items.length <= 2) return items.join(" and ");
  return `${items.slice(0, -1).join(", ")} and ${items[items.length - 1]}`;
}

export function renderCuts(root: HTMLElement, plan: CutListView): void {
  const names = plan.sections.filter((s) => s.on_hand).map((s) => s.name);
  const kept =
    plan.on_hand_boards.length === 0
      ? ""
      : `<h3 class="on-hand-heading">Already have: ${esc(list(names))}</h3>
         <p class="cuts-hint">These aren't on the shopping list. They're here in case you still need to cut them.</p>
         <div class="boards on-hand">${groupBoards(plan.on_hand_boards).map(board).join("")}</div>`;
  root.innerHTML = `
    <p class="cuts-hint">Pieces are listed in cutting order, measured from one end. Kerf is ${plan.summary.kerf}.</p>
    <div class="boards">${groupBoards(plan.boards).map(board).join("")}</div>
    ${kept}
    ${notes(plan, [])}
  `;
}

/** Boards cut the same way, together. */
function groupBoards(boards: BoardView[]): BoardView[][] {
  const groups = new Map<string, BoardView[]>();
  for (const b of boards) {
    const key = [b.size, b.length_16, ...b.pieces.map((p) => `${p.kind}:${p.length_16}`)].join("|");
    const group = groups.get(key);
    if (group) group.push(b);
    else groups.set(key, [b]);
  }
  return [...groups.values()];
}

function board(group: BoardView[]): string {
  const b = group[0];
  const ids = esc(group.length === 1 ? b.id : `${b.id} to ${group[group.length - 1].id}`);
  const count = group.length === 1 ? "" : `<span class="board-count">× ${group.length}</span>`;
  const bar = b.pieces
    .map((p) => {
      const pct = (p.length_16 / b.length_16) * 100;
      const text = pct > 9 ? `<span>${esc(p.length)}</span>` : "";
      return `<div class="cut kind-${p.kind}" style="width:${pct}%" title="${esc(p.label)}, ${esc(p.length)}">${text}</div>`;
    })
    .join("");
  const pieces = b.pieces
    .map((p, i) => {
      const labels = esc(group.map((g) => g.pieces[i].label).join(", "));
      return `<li><span class="num">${esc(p.length)}</span><span><span class="swatch kind-${p.kind}"></span>${KIND_NAMES[p.kind] ?? p.kind}</span><span class="labels">${labels}</span></li>`;
    })
    .join("");
  return `
    <article class="board">
      <header><strong>${ids}</strong><span>${b.size} × ${b.length}</span>${count}</header>
      <div class="board-bar">${bar}<div class="offcut"></div></div>
      <ol>${pieces}<li class="offcut-row"><span class="num">${b.offcut}</span><span>Offcut</span><span></span></li></ol>
    </article>
  `;
}

function notes(plan: CutListView, extra: string[]): string {
  const items = [...extra];
  for (const p of plan.too_long) {
    items.push(
      `${esc(p.label)} (${p.size} ${(KIND_NAMES[p.kind] ?? p.kind).toLowerCase()}, ${esc(p.length)}) is longer than any ${p.size} stock, so it isn't on the list. Add a longer stock length to cut it in one piece.`,
    );
  }
  for (const part of plan.skipped) {
    const name = part.charAt(0).toUpperCase() + part.slice(1);
    items.push(`${esc(name)} doesn't frame yet, so its pieces aren't included.`);
  }
  if (items.length === 0) return "";
  return `<ul class="warnings">${items.map((t) => `<li>${t}</li>`).join("")}</ul>`;
}
