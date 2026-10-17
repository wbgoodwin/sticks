// The shopping list and the board-by-board cut list, drawn from the core's
// plan. Boards cut the same way are shown once with a count.

import type { BoardView, CutListView } from "./core";
import { KIND_NAMES } from "./elevation";

function esc(text: string): string {
  return text.replace(/[&<>"]/g, (c) => `&${{ "&": "amp", "<": "lt", ">": "gt", '"': "quot" }[c]};`);
}

export function dollars(cents: number): string {
  return `$${(cents / 100).toFixed(2)}`;
}

export function renderShopping(root: HTMLElement, plan: CutListView): void {
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
    <table>
      <thead><tr><th>Qty</th><th>Size</th><th class="num">Length</th><th class="num">Each</th><th class="num">Total</th></tr></thead>
      <tbody>${rows}${total}</tbody>
    </table>
    ${notes(plan, priced ? [] : ["Add a price to every stock length in the lumber yard to see what this costs."])}
  `;
}

export function renderCuts(root: HTMLElement, plan: CutListView): void {
  const groups = new Map<string, BoardView[]>();
  for (const b of plan.boards) {
    const key = [b.size, b.length_16, ...b.pieces.map((p) => `${p.kind}:${p.length_16}`)].join("|");
    const group = groups.get(key);
    if (group) group.push(b);
    else groups.set(key, [b]);
  }
  root.innerHTML = `
    <p class="cuts-hint">Pieces are listed in cutting order, measured from one end. Kerf is ${plan.summary.kerf}.</p>
    <div class="boards">${[...groups.values()].map(board).join("")}</div>
    ${notes(plan, [])}
  `;
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
