// The plate layout card: every line to square across a wall's bottom and top
// plates before it goes together, measured from the layout end, with the
// letter framers write on the side the board goes: X stud, K king, J jack,
// C cripple.

import type { MemberKind, PlateBoardView, PlatesView, WallView } from "./core";

export const LETTER_NAMES: Record<string, string> = { X: "Stud", K: "King stud", J: "Jack stud", C: "Cripple" };

const PLATE_NAMES: Partial<Record<MemberKind, string>> = {
  bottom_plate: "Bottom plate",
  top_plate: "Top plate",
  cap_plate: "Cap plate",
};

export function isPlate(kind: string): boolean {
  return kind === "bottom_plate" || kind === "top_plate" || kind === "cap_plate";
}

function esc(text: string): string {
  return text.replace(/[&<>"]/g, (c) => `&${{ "&": "amp", "<": "lt", ">": "gt", '"': "quot" }[c]};`);
}

/** The card's body for one wall's plates. `width` is the room the plate picture has. */
export function platesHtml(wall: WallView, from: "left" | "right", width: number): string {
  const p = wall.plates;
  if (!p) return "";
  const end = `${from} end`;
  const how = p.top_sloped
    ? `<p>Hook your tape on the <strong>${end}</strong> of each plate. At each measurement, square a line across and write the letter on the far side of it, away from the ${end}: that's where the board stands.</p>
       <p class="plates-sloped">This wall's top plate follows the roof slope, so its marks are measured <strong>along the plate</strong> and come out a little longer than the bottom plate's. Mark the two plates separately, each from its own ${end}.</p>`
    : `<p>Lay the top plate on the bottom plate, ends flush at the <strong>${end}</strong>, and mark both at once. At each measurement, square a line across both and write the letter on the far side of it, away from the ${end}: that's where the board stands. Then add the marks only one plate gets.</p>`;
  return `
    <div class="plates-how">
      ${how}
      <ul class="plates-legend">${Object.entries(LETTER_NAMES)
        .map(([l, name]) => `<li><span class="letter letter-${l}">${l}</span>${name}</li>`)
        .join("")}</ul>
    </div>
    <div class="plates-strip">${stripSvg(wall, p, from, width)}</div>
    ${table(p, from)}
    ${splices(p)}
  `;
}

function badge(b: PlateBoardView | null): string {
  if (!b) return `<span class="none">—</span>`;
  return `<span class="letter letter-${b.letter}">${b.letter}</span><span class="plate-board">${LETTER_NAMES[b.letter]}</span><span class="plate-label">${esc(b.label)}</span>`;
}

/** Every line in order from the layout end, with each opening's rough opening between them. */
function table(p: PlatesView, from: "left" | "right"): string {
  type Row = { at: number; order: number; html: string };
  const rows: Row[] = [];
  for (const m of p.marks) {
    const cells = p.top_sloped
      ? `<td class="at">${m.bottom ? esc(m.at) : ""}</td><td>${badge(m.bottom)}</td><td class="at">${m.top ? esc(m.top_at) : ""}</td><td>${badge(m.top)}</td>`
      : `<td class="at">${esc(m.at)}</td><td>${badge(m.bottom)}</td><td>${badge(m.top)}</td>`;
    const one = m.bottom && m.top ? "" : " one-plate";
    rows.push({ at: m.at_16, order: 1, html: `<tr class="mark${one}">${cells}</tr>` });
  }
  const span = p.top_sloped ? 4 : 3;
  for (const o of p.openings) {
    const gap = o.no_bottom_plate ? " No bottom plate across it; the plate stops here." : "";
    rows.push({
      at: o.from_16,
      order: 0,
      html: `<tr class="opening-row"><td class="at">${esc(o.from)}</td><td colspan="${span - 1}"><strong>${esc(o.name)}</strong> rough opening starts.${gap}</td></tr>`,
    });
    rows.push({
      at: o.to_16,
      order: 0,
      html: `<tr class="opening-row"><td class="at">${esc(o.to)}</td><td colspan="${span - 1}"><strong>${esc(o.name)}</strong> rough opening ends.${o.no_bottom_plate ? " The bottom plate starts again." : ""}</td></tr>`,
    });
  }
  rows.sort((a, b) => a.at - b.at || a.order - b.order);
  const head = p.top_sloped
    ? `<tr><th class="at">From ${from} end</th><th>Bottom plate</th><th class="at">Along the slope</th><th>Top plate</th></tr>`
    : `<tr><th class="at">From ${from} end</th><th>Bottom plate</th><th>Top plate</th></tr>`;
  return `<table class="plates-table"><thead>${head}</thead><tbody>${rows.map((r) => r.html).join("")}</tbody></table>`;
}

function splices(p: PlatesView): string {
  if (p.splices.length === 0) return "";
  const items = p.splices.map((s) => `<li>${PLATE_NAMES[s.kind] ?? s.kind} joint centered at <strong>${esc(s.at)}</strong>, over a stud</li>`);
  return `<h3>Plate joints</h3><ul class="plates-splices">${items.join("")}</ul>`;
}

/** The two plates as they lie on the deck, top plate above, seen from the wall's outside face. */
function stripSvg(wall: WallView, p: PlatesView, from: "left" | "right", width: number): string {
  const L = wall.length_16 / 16;
  const rise = (wall.right_height_16 - wall.left_height_16) / 16;
  const topL = p.top_sloped ? L * Math.sqrt(1 + (rise / L) ** 2) : L;
  const pad = 28;
  // Big enough for a letter in each board; a long wall scrolls.
  const s = Math.max(8, (width - pad * 2) / topL);
  const w = Math.ceil(topL * s + pad * 2);
  const X = (inches: number) => pad + inches * s;
  const barH = 26;
  const topY = 22;
  const botY = topY + barH + 30;
  const roY = botY + barH + 12;
  const out: string[] = [];

  // Where a mark measured from the layout end lands, and which way its board goes.
  const place = (at16: number, length: number) => (from === "left" ? at16 / 16 : length - at16 / 16);
  const dir = from === "left" ? 1 : -1;
  const thick = (label: string) => (wall.members.find((m) => m.label === label)?.w_16 ?? 24) / 16;

  out.push(`<text class="plate-name" x="${X(0)}" y="${topY - 7}">Top plate${p.top_sloped ? ", along the slope" : ""}</text>`);
  out.push(`<rect class="plate" x="${X(0)}" y="${topY}" width="${topL * s}" height="${barH}"/>`);
  out.push(`<text class="plate-name" x="${X(0)}" y="${botY - 7}">Bottom plate</text>`);
  for (const m of wall.members.filter((v) => v.kind === "bottom_plate")) {
    out.push(`<rect class="plate" x="${X(m.x_16 / 16)}" y="${botY}" width="${(m.w_16 / 16) * s}" height="${barH}"/>`);
  }

  const board = (at16: number, length: number, y: number, b: PlateBoardView) => {
    const x = X(place(at16, length));
    const bw = thick(b.label) * s * dir;
    const left = Math.min(x, x + bw);
    const font = Math.min(13, Math.max(8, Math.abs(bw) + 2));
    out.push(
      `<g class="plate-mark letter-${b.letter}"><title>${LETTER_NAMES[b.letter]} ${esc(b.label)}</title>` +
        `<rect x="${left}" y="${y}" width="${Math.abs(bw)}" height="${barH}"/>` +
        `<line x1="${x}" y1="${y - 3}" x2="${x}" y2="${y + barH + 3}"/>` +
        `<text x="${left + Math.abs(bw) / 2}" y="${y + barH / 2}" font-size="${font}">${b.letter}</text></g>`,
    );
  };
  for (const m of p.marks) {
    if (m.top) board(m.top_at_16, topL, topY, m.top);
    if (m.bottom) board(m.at_16, L, botY, m.bottom);
  }
  for (const sp of p.splices) {
    if (sp.kind === "cap_plate") continue;
    const y = sp.kind === "bottom_plate" ? botY : topY;
    const x = X(place(sp.at_16, sp.kind === "bottom_plate" ? L : topL));
    out.push(`<line class="plate-splice" x1="${x}" y1="${y - 4}" x2="${x}" y2="${y + barH + 4}"><title>Joint</title></line>`);
  }
  for (const o of p.openings) {
    const a = X(place(o.from_16, L));
    const b = X(place(o.to_16, L));
    const [x1, x2] = [Math.min(a, b), Math.max(a, b)];
    out.push(
      `<g class="plate-ro"><line x1="${x1}" y1="${roY}" x2="${x2}" y2="${roY}"/><line x1="${x1}" y1="${roY - 5}" x2="${x1}" y2="${roY + 5}"/><line x1="${x2}" y1="${roY - 5}" x2="${x2}" y2="${roY + 5}"/>` +
        (x2 - x1 > o.name.length * 6.5 ? `<text x="${(x1 + x2) / 2}" y="${roY + 16}">${esc(o.name)}</text>` : "") +
        `</g>`,
    );
  }
  const arrowX = from === "left" ? X(0) : X(L);
  out.push(
    `<text class="plate-origin" x="${arrowX}" y="${roY + 34}" text-anchor="${from === "left" ? "start" : "end"}">${from === "left" ? "Measure from this end →" : "← Measure from this end"}</text>`,
  );
  const h = roY + 42;
  return `<svg class="plates-svg" viewBox="0 0 ${w} ${h}" width="${w}" height="${h}" role="img" aria-label="Plate layout">${out.join("")}</svg>`;
}
