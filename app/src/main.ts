import {
  cutList,
  drawing,
  formatLength,
  frame,
  loadCore,
  model,
  printPacket,
  projectFromToml,
  projectToToml,
  type FrameView,
  type HardwareView,
  type MemberView,
  type ProjectJson,
  type OpeningJson,
  type SolidView,
  type WallJson,
  type WallView,
} from "./core";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { mountCommand } from "./command";
import { dollars, renderCuts, renderShopping } from "./cutlist";
import { drawElevation, groupKey, KIND_NAMES, type DragOverlay, type Scale } from "./elevation";
import { openSection, revealOpening, renderPanel, wallName, type PanelContext, type PanelTab } from "./panel";
import { bounds, factsAt, movesUpAndDown, snap, type Item, type Spot } from "./snap";
import { renderSettings } from "./settings";
import { accountsEnabled, email, finishSignIn, me, onAccountChange, signedIn, type Me } from "./account";
import { renderAccount, statusText } from "./account-ui";
import { onSyncStatus, scheduleSync, setSyncEnabled, syncStatus, unlink } from "./sync";
import { ensureStock, fromTemplate, load, loadFiled, normalize, save, saveFiled, TEMPLATES } from "./state";
import { HIDDEN_LAYERS, LAYERS, Viewer } from "./view3d";

const app = document.querySelector<HTMLDivElement>("#app")!;
app.innerHTML = `
  <header class="topbar">
    <div class="brand"><span class="logo" aria-hidden="true"></span>Sticks</div>
    <input class="project-name" aria-label="Project name" spellcheck="false" autocomplete="off" />
    <div class="stats" aria-live="polite"></div>
    <div class="actions">
      <button type="button" class="button account-button" data-action="account" hidden></button>
      <button type="button" class="button icon-only" data-action="settings" aria-label="Settings" title="Settings: lumber and hardware prices, and saw kerf">
        <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M2 4h7M12 4h2M2 8h2M7 8h7M2 12h9"/><circle cx="10.5" cy="4" r="1.5"/><circle cx="5.5" cy="8" r="1.5"/><circle cx="12.5" cy="12" r="1.5"/></svg>
      </button>
      <details class="menu">
        <summary class="button">New</summary>
        <div class="menu-items" role="menu"></div>
      </details>
      <button type="button" class="button" data-action="open">Open…</button>
      <button type="button" class="button" data-action="save">Save</button>
      <button type="button" class="button primary" data-action="print">Print</button>
      <input type="file" accept=".toml,text/plain" hidden />
    </div>
  </header>
  <aside class="panel" id="design-panel" aria-label="Design"></aside>
  <div class="resizer" data-pane="panel" role="separator" aria-orientation="vertical" aria-controls="design-panel" aria-label="Resize design panel" tabindex="0"></div>
  <main class="stage">
    <div class="command"></div>
    <div class="viewbar">
      <button type="button" class="pane-toggle" data-pane="panel" aria-controls="design-panel"></button>
      <nav class="views" aria-label="Views"></nav>
    </div>
    <div class="drawing">
      <svg class="elevation" role="img" aria-label="Wall elevation"></svg>
      <div class="sheet" hidden></div>
      <div class="layers" hidden></div>
      <button type="button" class="button reset-view" hidden>Reset view</button>
      <div class="info" hidden></div>
      <div class="banner" hidden></div>
      <div class="tooltip" hidden></div>
    </div>
    <div class="resizer" data-pane="schedule" role="separator" aria-orientation="horizontal" aria-controls="schedule" aria-label="Resize lists" tabindex="0"></div>
    <section class="schedule" id="schedule">
      <div class="tabs" role="tablist" aria-label="Lists">
        <button type="button" role="tab" id="tab-members" aria-controls="members" aria-selected="true">Members</button>
        <button type="button" role="tab" id="tab-shopping" aria-controls="shopping" aria-selected="false">Shopping list</button>
        <button type="button" role="tab" id="tab-cuts" aria-controls="cuts" aria-selected="false">Cut list</button>
        <span class="tab-summary"></span>
        <button type="button" class="pane-toggle" data-pane="schedule" aria-controls="schedule"></button>
      </div>
      <div class="tab-panel" id="members" role="tabpanel" aria-labelledby="tab-members">
        <div class="legend"></div>
        <table>
          <thead><tr><th>Qty</th><th>Member</th><th>Size</th><th class="num">Length</th></tr></thead>
          <tbody></tbody>
        </table>
        <ul class="warnings"></ul>
      </div>
      <div class="tab-panel" id="shopping" role="tabpanel" aria-labelledby="tab-shopping" hidden></div>
      <div class="tab-panel" id="cuts" role="tabpanel" aria-labelledby="tab-cuts" hidden></div>
    </section>
  </main>
  <dialog class="settings" aria-labelledby="settings-title">
    <header>
      <div>
        <h2 id="settings-title">Lumber yard</h2>
        <p>What the shopping list can buy, and the hardware for it, saved with this project.</p>
      </div>
      <button type="button" class="icon-button" data-action="close-settings" aria-label="Close">×</button>
    </header>
    <div class="settings-body"></div>
  </dialog>
  <dialog class="settings account" aria-labelledby="account-title">
    <header>
      <div>
        <h2 id="account-title">Sticks account</h2>
        <p>Sync between your Macs, and plain-English changes.</p>
      </div>
      <button type="button" class="icon-button" data-action="close-account" aria-label="Close">×</button>
    </header>
    <div class="account-body"></div>
  </dialog>
  <dialog class="replace" aria-labelledby="replace-title">
    <form method="dialog">
      <h2 id="replace-title"></h2>
      <p>Changes since it was last saved to a file will be lost.</p>
      <div class="replace-actions">
        <button class="button" value="cancel">Cancel</button>
        <button class="button" value="replace">Don't save</button>
        <button class="button primary" value="save" autofocus>Save first…</button>
      </div>
    </form>
  </dialog>
  <div class="print-preview" hidden>
    <div class="print-bar">
      <strong>Build packet</strong>
      <span>Shopping list, every drawing and the cut list, on letter paper in landscape.</span>
      <button type="button" class="button" data-action="download-packet">Download</button>
      <button type="button" class="button primary" data-action="print-now">Print</button>
      <button type="button" class="button" data-action="close-preview">Close</button>
    </div>
    <iframe title="Build packet"></iframe>
  </div>
`;

const $ = <T extends Element>(selector: string) => app.querySelector<T>(selector)!;
const panel = $<HTMLElement>(".panel");
const views = $<HTMLElement>(".views");
const drawingBox = $<HTMLDivElement>(".drawing");
const svg = $<SVGSVGElement>(".elevation");
const sheet = $<HTMLDivElement>(".sheet");
const layers = $<HTMLDivElement>(".layers");
const resetView = $<HTMLButtonElement>(".reset-view");
const info = $<HTMLDivElement>(".info");
const banner = $<HTMLDivElement>(".banner");
const tooltip = $<HTMLDivElement>(".tooltip");
const tbody = $<HTMLTableSectionElement>("#members tbody");
const warnings = $<HTMLUListElement>("#members .warnings");
const shopping = $<HTMLDivElement>("#shopping");
const cuts = $<HTMLDivElement>("#cuts");
const tabSummary = $<HTMLSpanElement>(".tab-summary");
const stats = $<HTMLDivElement>(".stats");
const legend = $<HTMLDivElement>(".legend");
const nameInput = $<HTMLInputElement>(".project-name");
const fileInput = $<HTMLInputElement>('input[type="file"]');
const preview = $<HTMLDivElement>(".print-preview");
const settings = $<HTMLDialogElement>("dialog.settings");
const settingsBody = $<HTMLDivElement>(".settings-body");
const replaceDialog = $<HTMLDialogElement>("dialog.replace");
const accountDialog = $<HTMLDialogElement>("dialog.account");
const accountBody = $<HTMLDivElement>(".account-body");
const accountButton = $<HTMLButtonElement>(".account-button");
const previewFrame = $<HTMLIFrameElement>(".print-preview iframe");

/** What the stage shows: the 3D model, a wall's elevation, or a printed drawing. */
type View = { kind: "3d" } | { kind: "wall"; id: string } | { kind: "sheet"; id: string };

let project: ProjectJson;
let view: View = { kind: "3d" };
/** The wall the panel edits. */
let wall = "front";
/** The panel's tab, for a building. */
let tab: PanelTab = "building";
/** The last frame that worked, kept on screen while an edit doesn't. */
let framed: FrameView | null = null;
/** The last wall elevation drawn, redrawn when the stage resizes. */
let shownWall: WallView | null = null;

const viewer = new Viewer(drawingBox, {
  hover(solid, x, y) {
    if (!solid) {
      tooltip.hidden = true;
      return;
    }
    showTooltip(solidTitle(solid), x, y);
  },
  pick: showInfo,
});

const panelContext: PanelContext = {
  get wallIds() {
    return framed?.walls.map((w) => w.id) ?? project.wall.map((w) => w.id);
  },
  get wall() {
    return wall;
  },
  selectWall(id) {
    wall = id;
    tab = "walls";
    setView({ kind: "wall", id });
    renderPanel(panel, project, panelContext, events);
  },
  get tab() {
    return tab;
  },
  selectTab(next) {
    tab = next;
    renderPanel(panel, project, panelContext, events);
    liveHints();
  },
  wallLength(id) {
    const w = framed?.walls.find((v) => v.id === id);
    return w && !w.error ? w.length_16 : null;
  },
};

const events = {
  changed: update,
  restructured: () => {
    update();
    renderPanel(panel, project, panelContext, events);
    liveHints();
  },
};

function update(): void {
  let next: FrameView | null = null;
  let problem: unknown = null;
  try {
    next = frame(project);
  } catch (e) {
    problem = e;
  }
  const sizes = next ? allMembers(next).map((m) => m.size) : [];
  if (ensureStock(project, sizes)) renderPanel(panel, project, panelContext, events);
  save(project);
  scheduleSync(project);
  nameInput.value = project.name;
  if (next) {
    framed = next;
  } else {
    showProblem(message(problem));
    renderPlan();
    const hint = panel.querySelector<HTMLElement>('[data-live="back-height"]');
    if (hint) hint.textContent = "";
    return;
  }
  renderViews();
  renderStage();
  renderPlan();
  liveHints();
}

/** Text in the panel that depends on the framed result. */
function liveHints(): void {
  const back = framed?.walls.find((w) => w.id === "back");
  const hint = panel.querySelector<HTMLElement>('[data-live="back-height"]');
  if (hint) hint.textContent = back && !back.error && project.roof && project.roof.kind !== "gable" ? `The back wall comes out ${formatLength(back.left_height_16)}` : "";
}

function renderViews(): void {
  const list: [View, string][] = [[{ kind: "3d" }, "3D"]];
  for (const w of framed?.walls ?? []) list.push([{ kind: "wall", id: w.id }, wallName(w.id)]);
  for (const d of framed?.drawings ?? []) list.push([{ kind: "sheet", id: d.id }, d.name]);
  // A view that no longer exists (the roof was removed, say) falls back to 3D.
  if (!list.some(([v]) => sameView(v, view))) view = { kind: "3d" };
  const key = list.map(([v]) => viewKey(v)).join(",");
  if (views.dataset.key !== key) {
    views.dataset.key = key;
    views.replaceChildren(
      ...list.map(([v, text], i) => {
        const b = document.createElement("button");
        b.type = "button";
        b.textContent = text;
        b.dataset.view = viewKey(v);
        b.className = i > 0 && v.kind !== list[i - 1][0].kind ? "group-start" : "";
        b.addEventListener("click", () => {
          setView(v);
          // Picking a wall shows its openings in the panel.
          if (v.kind === "wall" && (v.id !== wall || tab !== "walls")) {
            wall = v.id;
            tab = "walls";
            renderPanel(panel, project, panelContext, events);
            liveHints();
          }
        });
        return b;
      }),
    );
  }
  for (const b of views.querySelectorAll<HTMLButtonElement>("button")) {
    b.setAttribute("aria-pressed", String(b.dataset.view === viewKey(view)));
  }
}

function viewKey(v: View): string {
  return v.kind === "3d" ? "3d" : `${v.kind}:${v.id}`;
}

function sameView(a: View, b: View): boolean {
  return viewKey(a) === viewKey(b);
}

function setView(v: View): void {
  view = v;
  tooltip.hidden = true;
  renderViews();
  renderStage();
}

function renderStage(): void {
  if (!framed) return;
  banner.hidden = true;
  drawingBox.classList.remove("stale");
  const is3d = view.kind === "3d";
  viewer.setActive(is3d);
  svg.style.display = view.kind === "wall" ? "" : "none";
  sheet.hidden = view.kind !== "sheet";
  layers.hidden = !is3d;
  resetView.hidden = !is3d;
  drawingBox.classList.toggle("plain", view.kind !== "wall");
  if (!is3d) info.hidden = true;

  if (view.kind === "3d") {
    const m = model(project);
    viewer.setModel(m);
    renderSchedule(allMembers(framed), [...framed.walls.flatMap((w) => w.warnings), ...planWarnings(framed)], skippedNotes(m.skipped));
    stats.textContent = `${m.solids.filter((s) => s.size).length} pieces`;
  } else if (view.kind === "wall") {
    const id = view.id;
    const w = framed.walls.find((v) => v.id === id);
    if (!w) return;
    if (w.error) {
      showProblem(`${wallName(w.id)} wall: ${w.error}`);
      return;
    }
    shownWall = w;
    drawWall();
    renderSchedule(w.members, w.warnings, []);
    stats.textContent = `${w.members.length} pieces · ${w.members.filter((m) => m.kind === "stud").length} studs`;
  } else {
    const id = view.id;
    const d = framed.drawings.find((v) => v.id === id);
    if (!d) return;
    const part = d.part === "sheathing" ? framed.sheathing : framed[d.part];
    if (part?.error) {
      showProblem(`${partName(d.part)}: ${part.error}`);
      return;
    }
    try {
      sheet.innerHTML = drawing(project, id);
    } catch (e) {
      showProblem(message(e));
      return;
    }
    if (part && "members" in part) {
      renderSchedule(part.members, part.warnings, []);
      stats.textContent = `${part.members.length} pieces`;
    } else {
      renderSchedule([], part?.warnings ?? [], []);
      stats.textContent = `${part?.pieces ?? 0} sheathing pieces`;
    }
  }
}

function allMembers(f: FrameView): MemberView[] {
  return [...f.walls.flatMap((w) => w.members), ...[f.floor, f.roof, f.porch].flatMap((p) => p?.members ?? [])];
}

function planWarnings(f: FrameView): string[] {
  return [f.floor, f.roof, f.porch, f.sheathing].flatMap((p) => p?.warnings ?? []);
}

function skippedNotes(parts: string[]): string[] {
  return parts.map((p) => `${capitalize(p)} doesn't frame, so it isn't shown.`);
}

/** How the shown wall was last drawn, to turn the pointer back into lengths. */
let scale: Scale | null = null;

function drawWall(drag?: DragOverlay): void {
  if (!shownWall || view.kind !== "wall") return;
  const { width, height } = drawingBox.getBoundingClientRect();
  const w = shownWall;
  const items = openingItems(w);
  const notes = items ? items.map((o, i) => factsAt(items, i, o, w.length_16).map((f) => f.text)) : undefined;
  scale = drawElevation(svg, w, { layoutFrom: project.rules.layout_from, notes, drag }, width, height);
}

/** The shown wall's openings with their kinds, or null if the project and the frame disagree. */
function openingItems(w: WallView): Item[] | null {
  const json = project.wall.find((v) => v.id === w.id);
  if (!json || json.opening.length !== w.openings.length) return null;
  return w.openings.map((o, i) => ({ name: o.name, kind: json.opening[i].kind, x: o.x_16, y: o.y_16, w: o.w_16, h: o.h_16 }));
}

function renderPlan(): void {
  try {
    const plan = cutList(project);
    renderShopping(shopping, plan, toggleOnHand);
    renderCuts(cuts, plan);
    const s = plan.summary;
    const hw = plan.hardware;
    const hwCost = hw.parts.length === 0 ? 0 : hw.total_cents;
    const cost = s.total_cents === null || hwCost === null ? "" : ` · ${dollars(s.total_cents + hwCost)}`;
    const sheets = s.sheets > 0 ? ` · ${s.sheets} sheets` : "";
    const had = plan.sections.filter((p) => p.on_hand).length;
    const kept = had === 0 ? "" : ` · ${had} on hand`;
    tabSummary.textContent = `${s.boards} boards${sheets} · ${s.waste_percent.toFixed(1)}% waste${cost}${kept}`;
  } catch (e) {
    const p = document.createElement("p");
    p.className = "plan-error";
    p.textContent = capitalize(message(e));
    shopping.replaceChildren(p);
    cuts.replaceChildren(p.cloneNode(true));
    tabSummary.textContent = "";
  }
}

/** The hardware list for Settings, or an empty one when the plan can't be made. */
function settingsHardware(): HardwareView {
  try {
    return cutList(project).hardware;
  } catch {
    return { brand: "Simpson Strong-Tie", parts: [], fasteners: [], warnings: [], total_cents: null };
  }
}

/** Marks a stage of the build had, or to buy again. */
function toggleOnHand(key: string): void {
  const had = project.on_hand ?? [];
  const next = had.includes(key) ? had.filter((k) => k !== key) : [...had, key];
  if (next.length > 0) project.on_hand = next;
  else delete project.on_hand;
  update();
}

function showProblem(text: string): void {
  banner.textContent = capitalize(text);
  banner.hidden = false;
  drawingBox.classList.add("stale");
}

function message(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

function capitalize(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}

function renderSchedule(members: MemberView[], notes: string[], extra: string[]): void {
  const groups = new Map<string, { member: MemberView; count: number }>();
  for (const m of members) {
    const key = groupKey(m);
    const g = groups.get(key);
    if (g) g.count++;
    else groups.set(key, { member: m, count: 1 });
  }
  const order = Object.keys(KIND_NAMES);
  const rows = [...groups.entries()].sort(
    ([, a], [, b]) =>
      order.indexOf(a.member.kind) - order.indexOf(b.member.kind) ||
      b.member.length_16 - a.member.length_16,
  );
  tbody.replaceChildren(
    ...rows.map(([key, { member, count }]) => {
      const tr = document.createElement("tr");
      tr.dataset.group = key;
      tr.innerHTML = `<td>${count}</td><td><span class="swatch kind-${member.kind}"></span>${KIND_NAMES[member.kind]}</td><td>${member.size}</td><td class="num">${member.length}</td>`;
      tr.addEventListener("mouseenter", () => highlight(key));
      tr.addEventListener("mouseleave", () => highlight(null));
      return tr;
    }),
  );

  const kinds = new Set(members.map((m) => m.kind));
  legend.innerHTML = [...kinds]
    .sort((a, b) => order.indexOf(a) - order.indexOf(b))
    .map((k) => `<span><span class="swatch kind-${k}"></span>${KIND_NAMES[k]}</span>`)
    .join("");

  warnings.replaceChildren(
    ...[...notes, ...extra].map((w) => {
      const li = document.createElement("li");
      li.textContent = capitalize(w);
      return li;
    }),
  );
}

function highlight(group: string | null): void {
  for (const node of svg.querySelectorAll<SVGElement>(".member")) {
    node.classList.toggle("highlight", group !== null && node.dataset.group === group);
  }
  svg.classList.toggle("highlighting", group !== null);
  for (const tr of tbody.querySelectorAll("tr")) {
    tr.classList.toggle("highlight", group !== null && tr.dataset.group === group);
  }
}

function showTooltip(html: string, x: number, y: number): void {
  tooltip.innerHTML = html;
  tooltip.hidden = false;
  const box = drawingBox.getBoundingClientRect();
  const left = Math.max(8, Math.min(x + 14, box.width - tooltip.offsetWidth - 8));
  const top = Math.max(8, y - tooltip.offsetHeight - 10);
  tooltip.style.transform = `translate(${left}px, ${top}px)`;
}

function kindName(s: SolidView): string {
  if (s.kind === "pier") return "Pier";
  if (s.kind === "subfloor") return "Subfloor sheet";
  if (s.kind === "sheathing") return "Sheathing";
  return KIND_NAMES[s.kind];
}

function partName(part: string): string {
  if (part === "floor") return "Floor";
  if (part === "roof") return "Roof";
  if (part === "porch") return "Porch";
  if (part === "deck") return "Porch deck";
  if (part === "sheathing") return "Sheathing";
  return `${wallName(part)} wall`;
}

function solidTitle(s: SolidView): string {
  const size = s.size && s.length ? `<span>${s.size} × ${s.length}</span>` : "";
  return `<strong>${esc(s.label)}</strong><span>${kindName(s)}, ${partName(s.part)}</span>${size}`;
}

function showInfo(s: SolidView | null): void {
  if (!s) {
    info.hidden = true;
    return;
  }
  const rows: [string, string][] = [[kindName(s), partName(s.part)]];
  if (s.size && s.length) rows.push(["Board", `${s.size} × ${s.length}`]);
  for (const [i, c] of s.cuts.entries()) rows.push([i === 0 ? "Cuts" : "", c]);
  if (s.size && s.cuts.length === 0) rows.push(["Cuts", "square both ends"]);
  info.innerHTML = `
    <header><strong>${esc(s.label)}</strong><button type="button" class="icon-button" aria-label="Close">×</button></header>
    <dl>${rows.map(([k, v]) => `<dt>${esc(k)}</dt><dd>${esc(v)}</dd>`).join("")}</dl>
  `;
  info.querySelector("button")!.addEventListener("click", () => {
    info.hidden = true;
    viewer.clearSelection();
  });
  info.hidden = false;
}

function esc(text: string): string {
  return text.replace(/[&<>"]/g, (c) => `&${{ "&": "amp", "<": "lt", ">": "gt", '"': "quot" }[c]};`);
}

svg.addEventListener("pointermove", (e) => {
  if (dragging) return;
  const handle = (e.target as Element).closest<SVGElement>(".opening-handle");
  if (handle && shownWall) {
    const o = shownWall.openings[Number(handle.dataset.index)];
    const box = drawingBox.getBoundingClientRect();
    const notes = svg.querySelectorAll(".rough-opening")[Number(handle.dataset.index)]?.classList.contains("ideal");
    showTooltip(
      `<strong>${esc(o.name)}</strong><span>${formatLength(o.x_16)} from the left end</span>${notes ? idealLines(Number(handle.dataset.index)) : ""}<span class="hint">Drag to move · hold ${ALT} to place freely</span>`,
      e.clientX - box.left,
      e.clientY - box.top,
    );
    highlight(null);
    return;
  }
  const target = (e.target as Element).closest<SVGElement>(".member");
  if (!target || !shownWall) {
    tooltip.hidden = true;
    highlight(null);
    return;
  }
  const m = shownWall.members.find((v) => v.label === target.dataset.label);
  if (!m) return;
  const plies = Number(target.dataset.plies ?? 1);
  const box = drawingBox.getBoundingClientRect();
  showTooltip(
    `<strong>${esc(m.label)}</strong><span>${KIND_NAMES[m.kind]}${plies > 1 ? `, ${plies} plies` : ""}</span><span>${m.size} × ${m.length}</span>`,
    e.clientX - box.left,
    e.clientY - box.top,
  );
  highlight(groupKey(m));
});
svg.addEventListener("pointerleave", () => {
  if (dragging) return;
  tooltip.hidden = true;
  highlight(null);
});

// Dragging an opening along its wall, framed live as it goes.
const ALT = /Mac|iPhone|iPad/.test(navigator.platform) ? "⌥" : "Alt";
/** Pixels the pointer must travel before a press becomes a drag. */
const DRAG_START = 4;
/** How close, in pixels, a good spot pulls the opening in. */
const SNAP_REACH = 10;
/** Pixels up or down before a window's height follows, so sliding it along doesn't nudge its sill. */
const VERTICAL_START = 12;

interface Drag {
  index: number;
  pointer: number;
  json: WallJson;
  opening: OpeningJson;
  /** The opening's place before the drag, to put back on Escape. */
  before: { offset: string; sill_height: string };
  start: Spot;
  startPx: { x: number; y: number };
  items: Item[];
  length: number;
  /** Where the frame last took it. */
  valid: Spot;
  moved: boolean;
  /** Whether it has been pulled far enough up or down to change height. */
  vertical: boolean;
  /** The pointer, in the stage's own coordinates. */
  at: { x: number; y: number };
  want: Spot | null;
  problem: string | null;
  queued: boolean;
}

let dragging: Drag | null = null;

function idealLines(index: number): string {
  const w = shownWall;
  const items = w && openingItems(w);
  if (!w || !items) return "";
  return factsAt(items, index, items[index], w.length_16)
    .map((f) => `<span class="good">✓ ${esc(f.text)}</span>`)
    .join("");
}

svg.addEventListener("pointerdown", (e) => {
  const handle = (e.target as Element).closest<SVGElement>(".opening-handle");
  if (!handle || !shownWall || e.button !== 0 || view.kind !== "wall") return;
  const index = Number(handle.dataset.index);
  const json = project.wall.find((v) => v.id === shownWall!.id);
  const items = openingItems(shownWall);
  if (!json || !items) return;
  const o = items[index];
  e.preventDefault();
  svg.setPointerCapture(e.pointerId);
  dragging = {
    index,
    pointer: e.pointerId,
    json,
    opening: json.opening[index],
    before: { offset: json.opening[index].offset, sill_height: json.opening[index].sill_height },
    start: { x: o.x, y: o.y },
    startPx: { x: e.clientX, y: e.clientY },
    items,
    length: shownWall.length_16,
    valid: { x: o.x, y: o.y },
    moved: false,
    vertical: false,
    at: { x: 0, y: 0 },
    want: null,
    problem: null,
    queued: false,
  };
});

svg.addEventListener("pointermove", (e) => {
  const d = dragging;
  if (!d || e.pointerId !== d.pointer || !scale) return;
  const dx = e.clientX - d.startPx.x;
  const dy = e.clientY - d.startPx.y;
  if (!d.moved && Math.hypot(dx, dy) < DRAG_START) return;
  if (!d.moved) {
    d.moved = true;
    svg.classList.add("dragging");
    highlight(null);
  }
  if (Math.abs(dy) >= VERTICAL_START) d.vertical = true;
  const raw = { x: d.start.x + (dx / scale.s) * 16, y: d.start.y - (dy / scale.s) * 16 };
  // Alt places it to the sixteenth; otherwise it pulls to good spots and whole inches.
  const free = e.altKey;
  const spot = snap(d.items, d.index, raw, d.length, free ? 0 : (SNAP_REACH / scale.s) * 16, free ? 1 : 16);
  d.want = d.vertical ? spot : { x: spot.x, y: d.start.y };
  const box = drawingBox.getBoundingClientRect();
  d.at = { x: e.clientX - box.left, y: e.clientY - box.top };
  if (!d.queued) {
    d.queued = true;
    requestAnimationFrame(reframeDrag);
  }
});

/** Frames the project with the opening where the pointer wants it, keeping the last spot that frames. */
function reframeDrag(): void {
  const d = dragging;
  if (!d || !d.want || !d.queued) return;
  d.queued = false;
  const want = d.want;
  place(d, want);
  let wall: WallView | undefined;
  let problem: string | null = null;
  try {
    const next = frame(project);
    wall = next.walls.find((w) => w.id === d.json.id);
    if (wall?.error) problem = wall.error;
    else if (wall) framed = next;
  } catch (err) {
    problem = message(err);
  }
  if (wall && !problem) {
    d.valid = want;
    d.problem = null;
    shownWall = wall;
  } else {
    place(d, d.valid);
    d.problem = problem ?? "it doesn't fit there";
  }
  d.items[d.index] = { ...d.items[d.index], x: d.valid.x, y: d.valid.y };
  const me = d.items[d.index];
  const facts = d.problem ? [] : factsAt(d.items, d.index, d.valid, d.length);
  const b = bounds(d.items, d.index, d.valid.x, d.length);
  drawWall({
    index: d.index,
    guides: facts.flatMap((f) => f.guides),
    gaps: [
      [b.left, me.x],
      [me.x + me.w, b.right],
    ],
    ghost: d.problem ? { x: want.x, y: want.y, w: me.w, h: me.h } : null,
  });
  const height = movesUpAndDown(me.kind) ? `<span>Sill at ${formatLength(d.valid.y)}</span>` : "";
  const lines = d.problem
    ? `<span class="bad">${esc(capitalize(d.problem))}</span>`
    : facts.map((f) => `<span class="good">✓ ${esc(f.text)}</span>`).join("");
  showTooltip(
    `<strong>${esc(me.name)}</strong><span>${formatLength(d.valid.x)} from the left end</span>${height}${lines}`,
    d.at.x,
    d.at.y,
  );
}

/** Writes a spot into the project, in the same notation the panel uses. */
function place(d: Drag, spot: Spot): void {
  d.opening.offset = formatLength(spot.x);
  if (movesUpAndDown(d.opening.kind)) d.opening.sill_height = formatLength(spot.y);
}

function endDrag(e: PointerEvent): void {
  const d = dragging;
  if (!d || e.pointerId !== d.pointer) return;
  // Let go before the next frame: frame the last move first.
  reframeDrag();
  dragging = null;
  svg.classList.remove("dragging");
  tooltip.hidden = true;
  if (!d.moved) {
    // A click: show the opening in the panel.
    revealInPanel(d.json.id, d.opening);
    return;
  }
  if (d.valid.x === d.start.x && d.valid.y === d.start.y) {
    Object.assign(d.opening, d.before);
    drawWall();
    return;
  }
  place(d, d.valid);
  events.restructured();
}
svg.addEventListener("pointerup", endDrag);
svg.addEventListener("pointercancel", cancelDrag);

/** Escape puts the opening back where it was. */
function cancelDrag(): void {
  const d = dragging;
  if (!d) return;
  dragging = null;
  svg.classList.remove("dragging");
  tooltip.hidden = true;
  if (svg.hasPointerCapture(d.pointer)) svg.releasePointerCapture(d.pointer);
  Object.assign(d.opening, d.before);
  update();
}
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && dragging) {
    e.preventDefault();
    cancelDrag();
  }
});

function revealInPanel(wallId: string, opening: OpeningJson): void {
  if (wall !== wallId || tab !== "walls") {
    wall = wallId;
    tab = "walls";
  }
  if (collapsed.panel) setCollapsed("panel", false);
  revealOpening(opening);
  renderPanel(panel, project, panelContext, events);
  liveHints();
  const card = panel.querySelector<HTMLElement>(".card.revealed");
  if (!card) return;
  card.scrollIntoView({ block: "center", behavior: "smooth" });
  card.classList.remove("flash");
  void card.offsetWidth;
  card.classList.add("flash");
}

new ResizeObserver(() => drawWall()).observe(drawingBox);

// Layer toggles over the 3D view.
layers.append(
  ...LAYERS.map(([layer, text]) => {
    const label = document.createElement("label");
    const box = document.createElement("input");
    box.type = "checkbox";
    box.checked = !HIDDEN_LAYERS.includes(layer);
    box.addEventListener("change", () => viewer.setLayer(layer, box.checked));
    label.append(box, text);
    return label;
  }),
);
resetView.addEventListener("click", () => viewer.resetView());

const tabs = [...app.querySelectorAll<HTMLButtonElement>('[role="tab"]')];
function selectTab(tab: HTMLButtonElement): void {
  for (const t of tabs) {
    const selected = t === tab;
    t.setAttribute("aria-selected", String(selected));
    t.tabIndex = selected ? 0 : -1;
    app.querySelector<HTMLElement>(`#${t.getAttribute("aria-controls")}`)!.hidden = !selected;
  }
}
for (const [i, tab] of tabs.entries()) {
  tab.tabIndex = i === 0 ? 0 : -1;
  tab.addEventListener("click", () => {
    // A tab on the folded-away lists opens them on that list.
    if (collapsed.schedule) setCollapsed("schedule", false);
    selectTab(tab);
  });
  tab.addEventListener("keydown", (e) => {
    const step = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
    if (!step) return;
    const next = tabs[(i + step + tabs.length) % tabs.length];
    selectTab(next);
    next.focus();
  });
}

// The design panel and the lists fold away to give the drawing the room.
type Pane = "panel" | "schedule";
const PANES_KEY = "sticks.collapsed";
const PANE_KEYS: Record<Pane, string> = { panel: "B", schedule: "J" };
const PANE_NAMES: Record<Pane, string> = { panel: "design panel", schedule: "lists" };
const mod = /Mac|iPhone|iPad/.test(navigator.platform) ? "⌘" : "Ctrl+";
const collapsed: Record<Pane, boolean> = { panel: false, schedule: false };
try {
  Object.assign(collapsed, JSON.parse(localStorage.getItem(PANES_KEY) ?? "{}"));
} catch {
  // Nothing remembered; both start open.
}
const paneToggles = [...app.querySelectorAll<HTMLButtonElement>(".pane-toggle")];
function setCollapsed(pane: Pane, value: boolean): void {
  collapsed[pane] = value;
  app.classList.toggle(`${pane}-collapsed`, value);
  for (const b of paneToggles) {
    if (b.dataset.pane !== pane) continue;
    const label = `${value ? "Show" : "Hide"} ${PANE_NAMES[pane]} (${mod}${PANE_KEYS[pane]})`;
    b.setAttribute("aria-expanded", String(!value));
    b.setAttribute("aria-label", label);
    b.title = label;
  }
  try {
    localStorage.setItem(PANES_KEY, JSON.stringify(collapsed));
  } catch {
    // Not remembered this time; the toggle still works.
  }
}
for (const b of paneToggles) {
  const pane = b.dataset.pane as Pane;
  b.innerHTML = pane === "panel"
    ? `<svg viewBox="0 0 16 16" aria-hidden="true"><rect x="1.5" y="2.5" width="13" height="11" rx="2"/><path d="M6 2.5v11"/></svg>`
    : `<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 6.5l4 4 4-4"/></svg>`;
  b.addEventListener("click", () => setCollapsed(pane, !collapsed[pane]));
  setCollapsed(pane, collapsed[pane]);
}
document.addEventListener("keydown", (e) => {
  if (!(e.metaKey || e.ctrlKey) || e.altKey || e.shiftKey) return;
  const pane = (Object.keys(PANE_KEYS) as Pane[]).find((p) => PANE_KEYS[p] === e.key.toUpperCase());
  if (!pane) return;
  e.preventDefault();
  setCollapsed(pane, !collapsed[pane]);
});

// Dragging a pane's inner edge sizes it; double-click puts it back. Sizes are
// a preference of this computer, not part of the project.
const SIZES_KEY = "sticks.sizes";
const SIZE_LIMITS: Record<Pane, { min: number; max: () => number; step: number }> = {
  panel: { min: 260, max: () => Math.max(260, Math.min(720, window.innerWidth * 0.6)), step: 20 },
  schedule: { min: 120, max: () => Math.max(120, window.innerHeight * 0.75), step: 20 },
};
const sizes: Partial<Record<Pane, number>> = {};
try {
  Object.assign(sizes, JSON.parse(localStorage.getItem(SIZES_KEY) ?? "{}"));
} catch {
  // Nothing remembered; both start at their usual size.
}
const resizers = [...app.querySelectorAll<HTMLElement>(".resizer")];
const paneBox = (pane: Pane) => (pane === "panel" ? panel : $<HTMLElement>(".schedule"));
function setSize(pane: Pane, px: number | undefined, remember = true): void {
  const limits = SIZE_LIMITS[pane];
  if (px === undefined) delete sizes[pane];
  else sizes[pane] = Math.round(Math.min(limits.max(), Math.max(limits.min, px)));
  const value = sizes[pane];
  const prop = pane === "panel" ? "--panel-width" : "--schedule-height";
  if (value === undefined) app.style.removeProperty(prop);
  else app.style.setProperty(prop, `${value}px`);
  app.classList.toggle(`${pane}-sized`, value !== undefined);
  for (const r of resizers) {
    if (r.dataset.pane !== pane) continue;
    r.setAttribute("aria-valuemin", String(limits.min));
    r.setAttribute("aria-valuemax", String(Math.round(limits.max())));
    r.setAttribute("aria-valuenow", String(Math.round(value ?? paneSize(pane))));
  }
  if (!remember) return;
  try {
    localStorage.setItem(SIZES_KEY, JSON.stringify(sizes));
  } catch {
    // Not remembered this time; the new size still holds.
  }
}
function paneSize(pane: Pane): number {
  const r = paneBox(pane).getBoundingClientRect();
  return pane === "panel" ? r.width : r.height;
}
for (const r of resizers) {
  const pane = r.dataset.pane as Pane;
  r.title = "Drag to resize, double-click to reset";
  r.addEventListener("pointerdown", (e) => {
    if (e.button !== 0) return;
    e.preventDefault();
    r.setPointerCapture(e.pointerId);
    const start = pane === "panel" ? e.clientX : e.clientY;
    const from = paneSize(pane);
    document.body.classList.add(pane === "panel" ? "resizing-x" : "resizing-y");
    r.classList.add("dragging");
    const move = (m: PointerEvent) => {
      // The panel grows rightward; the lists grow upward.
      const delta = pane === "panel" ? m.clientX - start : start - m.clientY;
      setSize(pane, from + delta, false);
    };
    const end = () => {
      r.removeEventListener("pointermove", move);
      r.removeEventListener("pointerup", end);
      r.removeEventListener("pointercancel", end);
      document.body.classList.remove("resizing-x", "resizing-y");
      r.classList.remove("dragging");
      setSize(pane, sizes[pane]);
    };
    r.addEventListener("pointermove", move);
    r.addEventListener("pointerup", end);
    r.addEventListener("pointercancel", end);
  });
  r.addEventListener("dblclick", () => setSize(pane, undefined));
  r.addEventListener("keydown", (e) => {
    const grow: Record<string, number> = pane === "panel" ? { ArrowRight: 1, ArrowLeft: -1 } : { ArrowUp: 1, ArrowDown: -1 };
    const dir = grow[e.key];
    if (dir) {
      e.preventDefault();
      setSize(pane, paneSize(pane) + dir * SIZE_LIMITS[pane].step * (e.shiftKey ? 3 : 1));
    } else if (e.key === "Home" || e.key === "End") {
      e.preventDefault();
      setSize(pane, e.key === "Home" ? SIZE_LIMITS[pane].min : SIZE_LIMITS[pane].max());
    } else if (e.key === "Enter") {
      e.preventDefault();
      setSize(pane, undefined);
    }
  });
  setSize(pane, sizes[pane], false);
}
// A smaller window can leave a remembered size too big; keep it in bounds.
window.addEventListener("resize", () => {
  for (const pane of Object.keys(sizes) as Pane[]) setSize(pane, sizes[pane], false);
});

// Project name, templates, files and printing.
nameInput.addEventListener("input", () => {
  project.name = nameInput.value;
  save(project);
  scheduleSync(project);
});
nameInput.addEventListener("change", update);

const menu = $<HTMLDetailsElement>(".menu");
$<HTMLDivElement>(".menu-items").append(
  ...TEMPLATES.map(([template, text]) => {
    const b = document.createElement("button");
    b.type = "button";
    b.role = "menuitem";
    b.textContent = text;
    b.addEventListener("click", async () => {
      menu.open = false;
      if (!(await okToReplace())) return;
      const next = fromTemplate(template);
      unlink();
      openProject(next);
      markFiled(next);
    });
    return b;
  }),
);
document.addEventListener("click", (e) => {
  if (!menu.contains(e.target as Node)) menu.open = false;
});

mountCommand($<HTMLDivElement>(".command"), {
  get project() {
    return project;
  },
  replace(next) {
    project = normalize(next);
    info.hidden = true;
    update();
    renderPanel(panel, project, panelContext, events);
    liveHints();
  },
  reveal(target) {
    const opening = /^(front|back|left|right)-\d+$/.exec(target);
    if (opening) panelContext.selectWall(opening[1]);
    const key = opening || target === "new_opening" ? "openings" : PANEL_SECTIONS[target];
    if (!key) return;
    if (key !== "openings") {
      tab = "building";
      openSection(key);
      renderPanel(panel, project, panelContext, events);
      liveHints();
    } else if (tab !== "walls") {
      panelContext.selectTab("walls");
    }
    const section = panel.querySelector<HTMLElement>(`[data-section="${key}"]`);
    if (!section) return;
    section.scrollIntoView({ block: "center", behavior: "smooth" });
    section.classList.remove("flash");
    void section.offsetWidth;
    section.classList.add("flash");
  },
});

/** The panel section where each command-bar target is edited. */
const PANEL_SECTIONS: Record<string, string> = {
  width: "shed",
  depth: "shed",
  height: "shed",
  footprint: "shed",
  studs: "framing",
  spacing: "framing",
  pitch: "roof",
  roof_shape: "roof",
  overhangs: "roof",
  rafters: "roof",
  joists: "floor",
  porch: "porch",
  porch_depth: "porch",
  deck: "porch",
};

/** Starts editing another project. */
function openProject(next: ProjectJson): void {
  project = next;
  framed = null;
  view = { kind: "3d" };
  wall = project.building ? "front" : (project.wall[0]?.id ?? "front");
  tab = "building";
  info.hidden = true;
  update();
  renderPanel(panel, project, panelContext, events);
  liveHints();
  viewer.resetView();
}

app.addEventListener("click", (e) => {
  const action = (e.target as Element).closest<HTMLElement>("[data-action]")?.dataset.action;
  switch (action) {
    case "open":
      okToReplace().then((ok) => ok && fileInput.click());
      break;
    case "save":
      saveProject();
      break;
    case "print":
      try {
        previewFrame.srcdoc = printPacket(project);
        preview.hidden = false;
      } catch (err) {
        showProblem(`Couldn't make the build packet: ${message(err)}`);
      }
      break;
    case "print-now":
      previewFrame.contentWindow?.print();
      break;
    case "download-packet":
      saveFile(`${fileName(project.name)}-build-packet.html`, previewFrame.srcdoc, "Web page", "html").catch((err) =>
        showProblem(`Couldn't save the build packet: ${message(err)}`),
      );
      break;
    case "close-preview":
      preview.hidden = true;
      break;
    case "settings":
      renderSettings(settingsBody, project, update, settingsHardware);
      settings.showModal();
      break;
    case "close-settings":
      settings.close();
      break;
    case "account":
      void renderAccount(accountBody, accountContext);
      accountDialog.showModal();
      break;
    case "close-account":
      accountDialog.close();
      break;
  }
});
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && !preview.hidden) preview.hidden = true;
});

// A click on the backdrop, outside the dialog's box, closes it.
settings.addEventListener("click", (e) => {
  if (e.target === settings) settings.close();
});
accountDialog.addEventListener("click", (e) => {
  if (e.target === accountDialog) accountDialog.close();
});

fileInput.addEventListener("change", async () => {
  const file = fileInput.files?.[0];
  fileInput.value = "";
  if (!file) return;
  try {
    const next = normalize(projectFromToml(await file.text()));
    unlink();
    openProject(next);
    markFiled(next);
  } catch (err) {
    showProblem(`${file.name} couldn't be read: ${message(err)}`);
  }
});

/**
 * Saves text to a file the person picks. The desktop window asks where with
 * the system's save dialog, since a download link does nothing there; the
 * browser downloads it. False when they cancel.
 */
async function saveFile(name: string, text: string, kind: string, extension: string): Promise<boolean> {
  if (isTauri()) return invoke<boolean>("save_file", { name, contents: text, kind, extension });
  const url = URL.createObjectURL(new Blob([text], { type: extension === "html" ? "text/html" : "text/plain" }));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
  return true;
}

/** Saves the project as a TOML file. False when it wasn't saved. */
async function saveProject(): Promise<boolean> {
  try {
    const text = projectToToml(project);
    if (!(await saveFile(`${fileName(project.name)}.toml`, text, "Sticks project", "toml"))) return false;
    saveFiled(text);
    return true;
  } catch (err) {
    showProblem(`Couldn't save: ${message(err)}`);
    return false;
  }
}

/** Notes that `p` matches a file, or a fresh template, so replacing it loses nothing. */
function markFiled(p: ProjectJson): void {
  try {
    saveFiled(projectToToml(p));
  } catch {
    // A project the core can't write out simply counts as unsaved.
  }
}

/** Whether the project has changes no file holds. */
function unfiled(): boolean {
  try {
    return projectToToml(project) !== loadFiled();
  } catch {
    return true;
  }
}

/**
 * Before New or Open replaces the project, offers to save changes no file
 * holds. True when it's fine to go ahead.
 */
async function okToReplace(): Promise<boolean> {
  if (!unfiled()) return true;
  $<HTMLHeadingElement>("#replace-title").textContent = `Save “${project.name.trim() || "this project"}” first?`;
  replaceDialog.returnValue = "cancel";
  replaceDialog.showModal();
  await new Promise((done) => replaceDialog.addEventListener("close", done, { once: true }));
  if (replaceDialog.returnValue === "replace") return true;
  if (replaceDialog.returnValue === "save") return saveProject();
  return false;
}

function fileName(name: string): string {
  return name.trim().toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "project";
}

// The Sticks account: signing in, the paid plan, and cloud sync.
let account: Me | null = null;

const accountContext = {
  async open(loadDesign: () => Promise<ProjectJson>) {
    // A project already syncing is safe in the account; anything else may need saving first.
    if (syncStatus().state !== "saved" && !(await okToReplace())) return;
    openProject(await loadDesign());
    markFiled(project);
    accountDialog.close();
  },
  refresh: refreshAccount,
};

/** Fetches the plan and turns sync on or off to match. */
async function refreshAccount(): Promise<Me | null> {
  account = signedIn() ? await me() : null;
  setSyncEnabled(Boolean(account?.paid), project);
  showAccount();
  return account;
}

function showAccount(): void {
  accountButton.hidden = !accountsEnabled;
  if (!accountsEnabled) return;
  const sync = syncStatus();
  accountButton.textContent = signedIn() ? (email() ?? "Account") : "Sign in";
  accountButton.dataset.sync = sync.state;
  accountButton.title = signedIn() ? statusText(sync) : "Sign in to sync designs and make changes in plain English";
}

onAccountChange(() => {
  refreshAccount().catch((e) => showProblem(message(e)));
  if (accountDialog.open) void renderAccount(accountBody, accountContext, false);
});
onSyncStatus((s) => {
  showAccount();
  const line = accountBody.querySelector<HTMLElement>("[data-sync-status]");
  if (line) line.textContent = statusText(s);
  // A first save adds the design to the dialog's list.
  if (accountDialog.open && (s.state === "saved" || s.state === "copied") && !accountBody.querySelector(".synced-list")) {
    void renderAccount(accountBody, accountContext);
  }
  if (s.state === "copied") showProblem(statusText(s));
});
// Back from the browser after paying: check the plan again.
window.addEventListener("focus", () => {
  if (signedIn() && !account?.paid) refreshAccount().catch(() => {});
});

loadCore().then(async () => {
  const saved = load();
  if (saved) openProject(saved);
  else {
    const fresh = fromTemplate("shed");
    openProject(fresh);
    markFiled(fresh);
  }
  showAccount();
  try {
    await finishSignIn();
    if (signedIn()) await refreshAccount();
  } catch (e) {
    showProblem(message(e));
  }
});
