// The Account dialog: sign in, the paid plan (upgrade, use this month, the
// extra-use cap, an end date, billing), and the designs synced to the account.

import { email, manageBilling, setBilling, signedIn, signIn, signOut, upgrade, type Me } from "./account";
import type { ProjectJson } from "./core";
import { h } from "./panel";
import { linkedId, listSynced, openSynced, syncStatus, type SyncStatus, type SyncedDesign } from "./sync";

export interface AccountContext {
  /** Opens a synced design in place of the current one, asking first if needed. */
  open(load: () => Promise<ProjectJson>): Promise<void>;
  /** The account's plan changed: refresh what depends on it. */
  refresh(): Promise<Me | null>;
}

let account: Me | null = null;
let designs: SyncedDesign[] | null = null;
let problem = "";
let waitingForBrowser = false;

/** Draws the dialog. Pass `reload` to fetch the plan and designs again first. */
export async function renderAccount(root: HTMLElement, ctx: AccountContext, reload = true): Promise<void> {
  if (!signedIn()) {
    account = null;
    designs = null;
  } else if (reload) {
    try {
      account = await ctx.refresh();
      designs = account?.paid ? await listSynced() : await listSynced().catch(() => null);
      problem = "";
    } catch (e) {
      problem = message(e);
    }
  }
  const redraw = (again = false) => void renderAccount(root, ctx, again);
  const act = (fn: () => Promise<unknown>, again = true) => async () => {
    try {
      problem = "";
      await fn();
    } catch (e) {
      problem = message(e);
    }
    redraw(again);
  };

  if (!signedIn()) {
    root.replaceChildren(
      h("p", { class: "account-lead" }, "Sign in to sync designs between your Macs and to make changes in plain English. Without an account, everything stays on this Mac."),
      ...present(problemLine()),
      h("div", { class: "account-actions" }, h("button", { type: "button", class: "button primary", onclick: act(signIn) }, "Sign in or create an account")),
      h("p", { class: "field-hint" }, "Sign-in opens in your browser and comes back here."),
    );
    return;
  }

  const sections: (HTMLElement | null)[] = [
    h("div", { class: "account-who" }, h("strong", {}, email() ?? "Signed in"), h("button", { type: "button", class: "button", onclick: act(signOut, false) }, "Sign out")),
    problemLine(),
  ];
  if (!account) {
    root.replaceChildren(...present(...sections));
    return;
  }

  if (!account.paid) {
    const ends = h("input", { type: "date", min: tomorrow(), "aria-label": "End date" });
    const endsWanted = h("input", { type: "checkbox", onchange: () => (ends.disabled = !endsWanted.checked) });
    ends.disabled = true;
    const j = account.jev;
    sections.push(
      h(
        "section",
        { class: "account-plan" },
        h("h3", {}, "Paid plan: $5 a month"),
        h(
          "ul",
          {},
          h("li", {}, "Your designs sync between your Macs."),
          h("li", {}, `Plain-English changes: ${j.included} a month included, then ${cents(j.overageCents)} each, up to a limit you set.`),
          h("li", {}, "Cancel any time, or pick the day it ends now."),
        ),
        h("label", { class: "check-field" }, endsWanted, "End my plan after ", ends),
        h(
          "div",
          { class: "account-actions" },
          h(
            "button",
            {
              type: "button",
              class: "button primary",
              onclick: act(async () => {
                await upgrade(endsWanted.checked && ends.value ? ends.value : undefined);
                waitingForBrowser = true;
              }, false),
            },
            "Upgrade",
          ),
          waitingForBrowser ? h("button", { type: "button", class: "button", onclick: act(async () => (waitingForBrowser = false)) }, "I've paid: check again") : null,
        ),
        waitingForBrowser ? h("p", { class: "field-hint" }, "Finish in your browser, then come back here.") : null,
        account.plan?.status === "canceled" ? h("p", { class: "field-hint" }, "Your plan has ended. Designs you synced can still be opened below.") : null,
      ),
    );
  } else {
    waitingForBrowser = false;
    sections.push(planSection(account, act), jevSection(account, act));
  }

  sections.push(syncedSection(ctx, act));
  root.replaceChildren(...present(...sections));
}

function planSection(a: Me, act: (fn: () => Promise<unknown>) => () => Promise<void>): HTMLElement {
  const p = a.plan;
  if (p?.source === "comped") {
    return h("section", { class: "account-plan" }, h("h3", {}, "Paid plan"), h("p", {}, "On the house: nothing to pay."));
  }
  const until = p?.paidUntil ? longDate(p.paidUntil) : "";
  const line = p?.renews === false ? `Ends ${until}.` : `Renews ${until}.`;
  const ends = h("input", { type: "date", min: tomorrow(), value: p?.endsOn ?? "", "aria-label": "End date" });
  return h(
    "section",
    { class: "account-plan" },
    h("h3", {}, "Paid plan: $5 a month"),
    h("p", {}, p?.status === "past_due" ? `${line} Your last payment didn't go through: check your card under Billing.` : line),
    h(
      "div",
      { class: "account-row" },
      h("span", { class: "field-label" }, "End after"),
      ends,
      h("button", { type: "button", class: "button", onclick: act(() => setBilling({ endsOn: ends.value || null })) }, "Save"),
      p?.endsOn ? h("button", { type: "button", class: "button", onclick: act(() => setBilling({ endsOn: null })) }, "Keep renewing") : null,
    ),
    h("p", { class: "field-hint" }, "It stops renewing after the month that includes this day, so you never pay for part of a month."),
    h("div", { class: "account-actions" }, h("button", { type: "button", class: "button", onclick: act(manageBilling) }, "Billing and receipts…")),
  );
}

function jevSection(a: Me, act: (fn: () => Promise<unknown>) => () => Promise<void>): HTMLElement {
  const j = a.jev;
  const comped = a.plan?.source === "comped";
  const cap = h("input", { type: "number", min: "0", max: "100", step: "1", value: String(j.capCents / 100), "aria-label": "Monthly limit for extra changes, in dollars" });
  return h(
    "section",
    { class: "account-plan" },
    h("h3", {}, "Plain-English changes"),
    h("meter", { min: "0", max: String(j.included), value: String(Math.min(j.used, j.included)) }),
    h("p", {}, `${j.used} of ${j.included} included this month.` + (j.extraCents > 0 ? ` ${dollars(j.extraCents)} extra so far.` : "")),
    comped
      ? null
      : h(
          "div",
          { class: "account-row" },
          h("span", { class: "field-label" }, "Spend at most $"),
          cap,
          h("span", { class: "field-label" }, "a month on extra changes"),
          h("button", { type: "button", class: "button", onclick: act(() => setBilling({ overageCapCents: Math.round(Number(cap.value) * 100) })) }, "Save"),
        ),
    comped ? null : h("p", { class: "field-hint" }, `Past the included ${j.included}, each change is ${cents(j.overageCents)}, added to your next bill. At your limit, changes wait for next month.`),
  );
}

function syncedSection(ctx: AccountContext, act: (fn: () => Promise<unknown>, again?: boolean) => () => Promise<void>): HTMLElement {
  const current = linkedId();
  const rows = (designs ?? []).map((d) =>
    h(
      "li",
      {},
      h("span", {}, h("strong", {}, d.name), h("em", {}, ` ${shortDate(d.updatedAt)}`)),
      d.id === current
        ? h("span", { class: "field-hint" }, "Open now")
        : h("button", { type: "button", class: "button", onclick: act(() => ctx.open(() => openSynced(d.id)), false) }, "Open"),
    ),
  );
  return h(
    "section",
    { class: "account-plan" },
    h("h3", {}, "Synced designs"),
    h("p", { class: "field-hint", "data-sync-status": "" }, statusText(syncStatus())),
    designs === null ? null : rows.length ? h("ul", { class: "synced-list" }, ...rows) : h("p", { class: "field-hint" }, "Nothing synced yet."),
  );
}

/** One line on what sync is doing, for the dialog and the top bar's title. */
export function statusText(s: SyncStatus): string {
  switch (s.state) {
    case "off":
      return "Sync is part of the paid plan.";
    case "saving":
      return "Saving to your account…";
    case "saved":
      return `Saved to your account at ${s.at.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}.`;
    case "offline":
      return "Offline: changes are kept on this Mac and sync when you're back.";
    case "copied":
      return `This design changed on another Mac, so your changes were saved as "${s.name}".`;
    case "error":
      return `Couldn't sync: ${s.message}`;
  }
}

function present(...els: (HTMLElement | null)[]): HTMLElement[] {
  return els.filter((e): e is HTMLElement => e !== null);
}

function problemLine(): HTMLElement | null {
  return problem ? h("p", { class: "field-error", role: "alert" }, problem) : null;
}

function message(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

function tomorrow(): string {
  return new Date(Date.now() + 86_400_000).toISOString().slice(0, 10);
}

function longDate(iso: string): string {
  return new Date(iso).toLocaleDateString([], { month: "long", day: "numeric", year: "numeric" });
}

function shortDate(iso: string): string {
  return new Date(iso).toLocaleDateString([], { month: "short", day: "numeric" });
}

function cents(n: number): string {
  return n < 100 ? `${n}¢` : dollars(n);
}

function dollars(n: number): string {
  return `$${(n / 100).toFixed(2)}`;
}
