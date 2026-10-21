// Cloud sync, for accounts on the paid plan: the open project saves to the
// account a couple of seconds after each change, and any synced design can
// be opened on another Mac.
//
// Each save names the version it started from. If another Mac saved the
// same design in between, the server refuses, and this copy is saved as a
// new design next to it: nothing is ever overwritten.

import { YardError, yard } from "./account";
import { projectFromToml, projectToToml, type ProjectJson } from "./core";
import { normalize } from "./state";

/** Which synced design the open project is, and the version this Mac last saw. */
interface Link {
  id: string;
  version: number;
  /** The TOML last saved, so an unchanged project isn't saved again. */
  toml: string;
}

export interface SyncedDesign {
  id: string;
  name: string;
  version: number;
  updatedAt: string;
}

export type SyncStatus =
  | { state: "off" }
  | { state: "saving" }
  | { state: "saved"; at: Date }
  | { state: "offline" }
  | { state: "copied"; name: string }
  | { state: "error"; message: string };

const LINK_KEY = "sticks.cloud.link";
const WAIT_MS = 2000;

let enabled = false;
let link: Link | null = readLink();
let status: SyncStatus = { state: "off" };
let timer: ReturnType<typeof setTimeout> | undefined;
let pending: ProjectJson | null = null;
let running: Promise<void> | null = null;
const listeners = new Set<(s: SyncStatus) => void>();

export function onSyncStatus(fn: (s: SyncStatus) => void): void {
  listeners.add(fn);
}

export function syncStatus(): SyncStatus {
  return status;
}

function setStatus(next: SyncStatus): void {
  status = next;
  for (const fn of listeners) fn(next);
}

/** Turns syncing on for a paid account, or off. Turning on saves the project now. */
export function setSyncEnabled(on: boolean, project?: ProjectJson): void {
  enabled = on;
  if (!on) {
    clearTimeout(timer);
    pending = null;
    setStatus({ state: "off" });
  } else if (project) {
    scheduleSync(project, 0);
  }
}

export function syncEnabled(): boolean {
  return enabled;
}

/** The open project is a new one (a template or a file): it syncs as a new design. */
export function unlink(): void {
  setLink(null);
}

/** Call after every change; saves once the edits pause. */
export function scheduleSync(project: ProjectJson, wait = WAIT_MS): void {
  if (!enabled) return;
  pending = project;
  clearTimeout(timer);
  timer = setTimeout(() => void flush(), wait);
}

/** Saves now whatever is waiting. */
export async function flush(): Promise<void> {
  // One save at a time, so versions go up in order.
  while (running) await running;
  const project = pending;
  pending = null;
  if (!project || !enabled) return;
  running = push(project).finally(() => (running = null));
  await running;
}

async function push(project: ProjectJson): Promise<void> {
  const toml = projectToToml(project);
  if (link && link.toml === toml) return;
  setStatus({ state: "saving" });
  const name = project.name.trim() || "Untitled";
  try {
    if (!link) {
      const saved = await put(newId(), name, toml, 0);
      setLink({ id: saved.id, version: saved.version, toml });
    } else {
      try {
        const saved = await put(link.id, name, toml, link.version);
        setLink({ ...link, version: saved.version, toml });
      } catch (e) {
        if (!(e instanceof YardError) || e.status !== 409) throw e;
        // Changed on another Mac since this one last saw it: keep both.
        const copy = `${name} (copy)`;
        const saved = await put(newId(), copy, toml, 0);
        setLink({ id: saved.id, version: saved.version, toml });
        setStatus({ state: "copied", name: copy });
        return;
      }
    }
    setStatus({ state: "saved", at: new Date() });
  } catch (e) {
    if (e instanceof YardError && e.status === 0) {
      // Try again in a bit; the next edit also retries.
      setStatus({ state: "offline" });
      if (!pending) scheduleSync(project, 30_000);
    } else if (e instanceof YardError && (e.status === 401 || e.status === 402)) {
      setSyncEnabled(false);
    } else {
      setStatus({ state: "error", message: e instanceof Error ? e.message : String(e) });
    }
  }
}

function put(id: string, name: string, toml: string, baseVersion: number) {
  return yard<SyncedDesign>("PUT", `/designs/${encodeURIComponent(id)}`, { name, toml, baseVersion });
}

/** The account's synced designs, newest first. */
export async function listSynced(): Promise<SyncedDesign[]> {
  return (await yard<{ designs: SyncedDesign[] }>("GET", "/designs")).designs;
}

/** Opens a synced design; later changes save back to it. */
export async function openSynced(id: string): Promise<ProjectJson> {
  await flush();
  const design = await yard<SyncedDesign & { toml: string }>("GET", `/designs/${encodeURIComponent(id)}`);
  const project = normalize(projectFromToml(design.toml));
  setLink({ id: design.id, version: design.version, toml: projectToToml(project) });
  return project;
}

/** The synced design the open project is, if any. */
export function linkedId(): string | null {
  return link?.id ?? null;
}

function setLink(next: Link | null): void {
  link = next;
  try {
    if (next) localStorage.setItem(LINK_KEY, JSON.stringify(next));
    else localStorage.removeItem(LINK_KEY);
  } catch {
    // Without it the next save starts a new synced design; nothing is lost.
  }
}

function readLink(): Link | null {
  try {
    const saved = localStorage.getItem(LINK_KEY);
    return saved ? (JSON.parse(saved) as Link) : null;
  } catch {
    return null;
  }
}

function newId(): string {
  return crypto.randomUUID();
}
