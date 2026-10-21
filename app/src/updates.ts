// Tells the user when a newer Sticks is out.
//
// Sticks is installed by building it from source, so an update is a
// `git pull` and a rebuild. On start the app asks GitHub for the latest
// release at most once a day and, if it's newer than this build, shows a
// card saying so and how to update. Skipping a version keeps it quiet until
// the next one.

import { isTauri } from "@tauri-apps/api/core";

const REPO = "wbgoodwin/sticks";
const KEY = "sticks.updates";
const DAY_MS = 24 * 60 * 60 * 1000;

/** The version this build was made from, from tauri.conf.json. */
export const VERSION: string = __APP_VERSION__;

interface Release {
  version: string;
  url: string;
}

interface Remembered {
  /** When GitHub was last asked, in ms. */
  checked?: number;
  latest?: Release;
  /** The version the user said not to remind them about. */
  skipped?: string;
}

function remembered(): Remembered {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? "{}") as Remembered;
  } catch {
    return {};
  }
}

function remember(r: Remembered): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(r));
  } catch {
    // Not remembering only means asking GitHub again next start.
  }
}

/** -1, 0 or 1 as `a` is older, the same as or newer than `b` ("0.2.0", "v1.10.3"). */
export function compareVersions(a: string, b: string): number {
  const parts = (v: string) =>
    v
      .replace(/^v/, "")
      .split("-")[0]
      .split(".")
      .map((n) => Number.parseInt(n, 10) || 0);
  const [x, y] = [parts(a), parts(b)];
  for (let i = 0; i < Math.max(x.length, y.length); i++) {
    const d = (x[i] ?? 0) - (y[i] ?? 0);
    if (d !== 0) return Math.sign(d);
  }
  return 0;
}

/** The latest published release, or null when there is none yet. */
async function latestRelease(): Promise<Release | null> {
  const res = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`, {
    headers: { Accept: "application/vnd.github+json" },
  });
  if (res.status === 404) return null;
  if (!res.ok) throw new Error(`GitHub answered ${res.status}`);
  const body = (await res.json()) as { tag_name: string; html_url: string };
  return { version: body.tag_name.replace(/^v/, ""), url: body.html_url };
}

/** Opens a web page in the user's browser, not in the app's window. */
async function openPage(url: string): Promise<void> {
  if (isTauri()) {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(url);
  } else {
    window.open(url, "_blank", "noopener");
  }
}

/**
 * Checks for a newer release (at most daily) and shows `card` if there is
 * one. Quiet when offline or when GitHub can't be reached: it tries again on
 * the next start.
 */
export async function checkForUpdates(card: HTMLElement): Promise<void> {
  const saved = remembered();
  let latest = saved.latest;
  if (!saved.checked || Date.now() - saved.checked > DAY_MS) {
    try {
      latest = (await latestRelease()) ?? undefined;
      remember({ ...saved, checked: Date.now(), latest });
    } catch {
      return;
    }
  }
  if (!latest || compareVersions(latest.version, VERSION) <= 0 || saved.skipped === latest.version) return;
  showUpdate(card, latest);
}

function showUpdate(card: HTMLElement, release: Release): void {
  card.innerHTML = `
    <div>
      <strong></strong>
      <p>You have ${VERSION}. To update, run <code>git pull</code> and then <code>npm run app:install</code> in the <code>app</code> folder of your Sticks copy.</p>
    </div>
    <div class="update-actions">
      <button type="button" class="button" data-do="skip">Skip this version</button>
      <button type="button" class="button primary" data-do="notes">What's new</button>
    </div>
  `;
  card.querySelector("strong")!.textContent = `Sticks ${release.version} is out`;
  card.hidden = false;
  card.addEventListener("click", (e) => {
    const action = (e.target as Element).closest<HTMLElement>("[data-do]")?.dataset.do;
    if (action === "notes") void openPage(release.url);
    if (action === "skip") {
      remember({ ...remembered(), skipped: release.version });
      card.hidden = true;
    }
  });
}
