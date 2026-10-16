import { frame, loadCore, parseLength, type MemberView, type ProjectJson, type WallView } from "./core";
import { drawElevation, groupKey, KIND_NAMES, type OpeningOutline } from "./elevation";
import { renderPanel } from "./panel";
import { load, save } from "./state";

const app = document.querySelector<HTMLDivElement>("#app")!;
app.innerHTML = `
  <header class="topbar">
    <div class="brand"><span class="logo" aria-hidden="true"></span>Sticks</div>
    <div class="project-name"></div>
    <div class="stats" aria-live="polite"></div>
  </header>
  <aside class="panel" aria-label="Wall settings"></aside>
  <main class="stage">
    <div class="drawing">
      <svg class="elevation" role="img" aria-label="Wall elevation"></svg>
      <div class="banner" hidden></div>
      <div class="tooltip" hidden></div>
    </div>
    <section class="schedule" aria-label="Members">
      <div class="legend"></div>
      <table>
        <thead><tr><th>Qty</th><th>Member</th><th>Size</th><th class="num">Length</th></tr></thead>
        <tbody></tbody>
      </table>
      <ul class="warnings"></ul>
    </section>
  </main>
`;

const panel = app.querySelector<HTMLElement>(".panel")!;
const drawing = app.querySelector<HTMLDivElement>(".drawing")!;
const svg = app.querySelector<SVGSVGElement>(".elevation")!;
const banner = app.querySelector<HTMLDivElement>(".banner")!;
const tooltip = app.querySelector<HTMLDivElement>(".tooltip")!;
const tbody = app.querySelector<HTMLTableSectionElement>(".schedule tbody")!;
const warnings = app.querySelector<HTMLUListElement>(".warnings")!;
const stats = app.querySelector<HTMLDivElement>(".stats")!;
const legend = app.querySelector<HTMLDivElement>(".legend")!;

let project: ProjectJson = load();
/** The last wall that framed, kept on screen while an edit doesn't. */
let shown: { wall: WallView; openings: OpeningOutline[]; layoutFrom: "left" | "right" } | null = null;

function update(): void {
  save(project);
  app.querySelector(".project-name")!.textContent = project.name;
  let wall: WallView;
  try {
    wall = frame(project)[0];
  } catch (e) {
    showProblem(e instanceof Error ? e.message : String(e));
    return;
  }
  if (wall.error) {
    showProblem(wall.error);
    return;
  }
  banner.hidden = true;
  drawing.classList.remove("stale");
  shown = { wall, openings: outlines(project), layoutFrom: project.rules.layout_from };
  draw();
  renderSchedule(wall);
}

function showProblem(message: string): void {
  banner.textContent = message.charAt(0).toUpperCase() + message.slice(1);
  banner.hidden = false;
  drawing.classList.toggle("stale", shown !== null);
}

function outlines(p: ProjectJson): OpeningOutline[] {
  const names = { door: "Door", window: "Window", penetration: "Penetration" };
  return p.wall[0].opening.flatMap((o, i) => {
    const values = [o.offset, o.sill_height, o.width, o.height].map(parseLength);
    if (!values.every((v) => "ok" in v)) return [];
    const [x16, y16, w16, h16] = values.map((v) => ("ok" in v ? v.ok : 0));
    return [{ name: `${names[o.kind]} ${i + 1}`, x16, y16, w16, h16 }];
  });
}

function draw(): void {
  if (!shown) return;
  const { width, height } = drawing.getBoundingClientRect();
  drawElevation(svg, shown.wall, shown, width, height);
}

function renderSchedule(wall: WallView): void {
  const groups = new Map<string, { member: MemberView; count: number }>();
  for (const m of wall.members) {
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

  const kinds = new Set(wall.members.map((m) => m.kind));
  legend.innerHTML = [...kinds]
    .sort((a, b) => order.indexOf(a) - order.indexOf(b))
    .map((k) => `<span><span class="swatch kind-${k}"></span>${KIND_NAMES[k]}</span>`)
    .join("");

  const studs = wall.members.filter((m) => m.kind === "stud").length;
  stats.textContent = `${wall.members.length} pieces · ${studs} studs`;

  warnings.replaceChildren(
    ...wall.warnings.map((w) => {
      const li = document.createElement("li");
      li.textContent = w.charAt(0).toUpperCase() + w.slice(1);
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

svg.addEventListener("pointermove", (e) => {
  const target = (e.target as Element).closest<SVGElement>(".member");
  if (!target || !shown) {
    tooltip.hidden = true;
    highlight(null);
    return;
  }
  const m = shown.wall.members.find((v) => v.label === target.dataset.label);
  if (!m) return;
  const plies = Number(target.dataset.plies ?? 1);
  tooltip.innerHTML = `<strong>${m.label}</strong><span>${KIND_NAMES[m.kind]}${plies > 1 ? `, ${plies} plies` : ""}</span><span>${m.size} × ${m.length}</span>`;
  tooltip.hidden = false;
  const box = drawing.getBoundingClientRect();
  const x = Math.min(e.clientX - box.left + 14, box.width - tooltip.offsetWidth - 8);
  const y = Math.max(8, e.clientY - box.top - tooltip.offsetHeight - 10);
  tooltip.style.transform = `translate(${x}px, ${y}px)`;
  highlight(groupKey(m));
});
svg.addEventListener("pointerleave", () => {
  tooltip.hidden = true;
  highlight(null);
});

new ResizeObserver(draw).observe(drawing);

const events = {
  changed: update,
  restructured: () => {
    renderPanel(panel, project, events);
    update();
  },
};

loadCore().then(() => {
  renderPanel(panel, project, events);
  update();
});
