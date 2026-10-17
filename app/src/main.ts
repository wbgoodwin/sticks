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
  type MemberView,
  type ProjectJson,
  type SolidView,
  type WallView,
} from "./core";
import { dollars, renderCuts, renderShopping } from "./cutlist";
import { drawElevation, groupKey, KIND_NAMES } from "./elevation";
import { renderPanel, wallName, type PanelContext } from "./panel";
import { ensureStock, fromTemplate, load, normalize, save, TEMPLATES } from "./state";
import { LAYERS, Viewer } from "./view3d";

const app = document.querySelector<HTMLDivElement>("#app")!;
app.innerHTML = `
  <header class="topbar">
    <div class="brand"><span class="logo" aria-hidden="true"></span>Sticks</div>
    <input class="project-name" aria-label="Project name" spellcheck="false" autocomplete="off" />
    <div class="stats" aria-live="polite"></div>
    <div class="actions">
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
  <aside class="panel" aria-label="Design"></aside>
  <main class="stage">
    <nav class="views" aria-label="Views"></nav>
    <div class="drawing">
      <svg class="elevation" role="img" aria-label="Wall elevation"></svg>
      <div class="sheet" hidden></div>
      <div class="layers" hidden></div>
      <button type="button" class="button reset-view" hidden>Reset view</button>
      <div class="info" hidden></div>
      <div class="banner" hidden></div>
      <div class="tooltip" hidden></div>
    </div>
    <section class="schedule">
      <div class="tabs" role="tablist" aria-label="Lists">
        <button type="button" role="tab" id="tab-members" aria-controls="members" aria-selected="true">Members</button>
        <button type="button" role="tab" id="tab-shopping" aria-controls="shopping" aria-selected="false">Shopping list</button>
        <button type="button" role="tab" id="tab-cuts" aria-controls="cuts" aria-selected="false">Cut list</button>
        <span class="tab-summary"></span>
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
const previewFrame = $<HTMLIFrameElement>(".print-preview iframe");

/** What the stage shows: the 3D model, a wall's elevation, or a printed drawing. */
type View = { kind: "3d" } | { kind: "wall"; id: string } | { kind: "sheet"; id: "floor" | "subfloor" | "roof" | "rafter" };

const SHEETS = { floor: "Floor", subfloor: "Subfloor", roof: "Roof", rafter: "Rafter" } as const;

let project: ProjectJson;
let view: View = { kind: "3d" };
/** The wall the panel edits. */
let wall = "front";
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
    setView({ kind: "wall", id });
    renderPanel(panel, project, panelContext, events);
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
  if (ensureStock(project)) renderPanel(panel, project, panelContext, events);
  save(project);
  nameInput.value = project.name;
  try {
    framed = frame(project);
  } catch (e) {
    showProblem(message(e));
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
  if (hint) hint.textContent = back && !back.error && project.roof ? `The back wall comes out ${formatLength(back.left_height_16)}` : "";
}

function renderViews(): void {
  const list: [View, string][] = [[{ kind: "3d" }, "3D"]];
  for (const w of framed?.walls ?? []) list.push([{ kind: "wall", id: w.id }, wallName(w.id)]);
  if (framed?.floor) list.push([{ kind: "sheet", id: "floor" }, SHEETS.floor], [{ kind: "sheet", id: "subfloor" }, SHEETS.subfloor]);
  if (framed?.roof) list.push([{ kind: "sheet", id: "roof" }, SHEETS.roof], [{ kind: "sheet", id: "rafter" }, SHEETS.rafter]);
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
          if (v.kind === "wall" && v.id !== wall) {
            wall = v.id;
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
    const part = view.id === "floor" || view.id === "subfloor" ? framed.floor : framed.roof;
    if (part?.error) {
      showProblem(`${view.id === "floor" || view.id === "subfloor" ? "Floor" : "Roof"}: ${part.error}`);
      return;
    }
    try {
      sheet.innerHTML = drawing(project, view.id);
    } catch (e) {
      showProblem(message(e));
      return;
    }
    renderSchedule(part?.members ?? [], part?.warnings ?? [], []);
    stats.textContent = `${part?.members.length ?? 0} pieces`;
  }
}

function allMembers(f: FrameView): MemberView[] {
  return [...f.walls.flatMap((w) => w.members), ...(f.floor?.members ?? []), ...(f.roof?.members ?? [])];
}

function planWarnings(f: FrameView): string[] {
  return [...(f.floor?.warnings ?? []), ...(f.roof?.warnings ?? [])];
}

function skippedNotes(parts: string[]): string[] {
  return parts.map((p) => `${capitalize(p)} doesn't frame, so it isn't shown.`);
}

function drawWall(): void {
  if (!shownWall || view.kind !== "wall") return;
  const { width, height } = drawingBox.getBoundingClientRect();
  drawElevation(svg, shownWall, { layoutFrom: project.rules.layout_from }, width, height);
}

function renderPlan(): void {
  try {
    const plan = cutList(project);
    renderShopping(shopping, plan);
    renderCuts(cuts, plan);
    const s = plan.summary;
    const cost = s.total_cents === null ? "" : ` · ${dollars(s.total_cents)}`;
    const sheets = s.sheets > 0 ? ` · ${s.sheets} sheets` : "";
    tabSummary.textContent = `${s.boards} boards${sheets} · ${s.waste_percent.toFixed(1)}% waste${cost}`;
  } catch (e) {
    const p = document.createElement("p");
    p.className = "plan-error";
    p.textContent = capitalize(message(e));
    shopping.replaceChildren(p);
    cuts.replaceChildren(p.cloneNode(true));
    tabSummary.textContent = "";
  }
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
  const left = Math.min(x + 14, box.width - tooltip.offsetWidth - 8);
  const top = Math.max(8, y - tooltip.offsetHeight - 10);
  tooltip.style.transform = `translate(${left}px, ${top}px)`;
}

function kindName(s: SolidView): string {
  if (s.kind === "pier") return "Pier";
  if (s.kind === "subfloor") return "Subfloor sheet";
  return KIND_NAMES[s.kind];
}

function partName(part: string): string {
  if (part === "floor") return "Floor";
  if (part === "roof") return "Roof";
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
  tooltip.hidden = true;
  highlight(null);
});

new ResizeObserver(drawWall).observe(drawingBox);

// Layer toggles over the 3D view.
layers.append(
  ...LAYERS.map(([layer, text]) => {
    const label = document.createElement("label");
    const box = document.createElement("input");
    box.type = "checkbox";
    box.checked = true;
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
  tab.addEventListener("click", () => selectTab(tab));
  tab.addEventListener("keydown", (e) => {
    const step = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
    if (!step) return;
    const next = tabs[(i + step + tabs.length) % tabs.length];
    selectTab(next);
    next.focus();
  });
}

// Project name, templates, files and printing.
nameInput.addEventListener("input", () => {
  project.name = nameInput.value;
  save(project);
});
nameInput.addEventListener("change", update);

const menu = $<HTMLDetailsElement>(".menu");
$<HTMLDivElement>(".menu-items").append(
  ...TEMPLATES.map(([template, text]) => {
    const b = document.createElement("button");
    b.type = "button";
    b.role = "menuitem";
    b.textContent = text;
    b.addEventListener("click", () => {
      menu.open = false;
      openProject(fromTemplate(template));
    });
    return b;
  }),
);
document.addEventListener("click", (e) => {
  if (!menu.contains(e.target as Node)) menu.open = false;
});

/** Starts editing another project. */
function openProject(next: ProjectJson): void {
  project = next;
  framed = null;
  view = { kind: "3d" };
  wall = project.building ? "front" : (project.wall[0]?.id ?? "front");
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
      fileInput.click();
      break;
    case "save":
      try {
        download(`${fileName(project.name)}.toml`, projectToToml(project), "text/plain");
      } catch (err) {
        showProblem(`Couldn't save: ${message(err)}`);
      }
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
      download(`${fileName(project.name)}-build-packet.html`, previewFrame.srcdoc, "text/html");
      break;
    case "close-preview":
      preview.hidden = true;
      break;
  }
});
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && !preview.hidden) preview.hidden = true;
});

fileInput.addEventListener("change", async () => {
  const file = fileInput.files?.[0];
  fileInput.value = "";
  if (!file) return;
  try {
    openProject(normalize(projectFromToml(await file.text())));
  } catch (err) {
    showProblem(`${file.name} couldn't be read: ${message(err)}`);
  }
});

function download(name: string, text: string, type: string): void {
  const url = URL.createObjectURL(new Blob([text], { type }));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

function fileName(name: string): string {
  return name.trim().toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "project";
}

loadCore().then(() => {
  openProject(load() ?? fromTemplate("shed"));
});
