// The command bar: a change in plain English, previewed before it's made.
//
// Rust asks Jev what in the design each part of the request changes, then
// that thing's own questions (what to do to it, where, how many), and turns
// the confident answers into changes. Whatever Jev wasn't sure of, or the
// request doesn't say, shows as buttons for the user. An answer from the
// user may open new questions for Jev, such as once they pick what changes.
// Without Jev, a model on this machine answers the same questions (see
// answerer.ts).

import { cutList, describeDesign, editQuestions, readEdit, type EditAnswer, type EditReading, type ProjectJson } from "./core";
import { dollars } from "./cutlist";
import { ask } from "./answerer";
import { normalize } from "./state";

export interface CommandContext {
  readonly project: ProjectJson;
  /** Makes `next` the project, keeping the view. */
  replace(next: ProjectJson): void;
  /** Shows where a target (`width`, `front-1`, ...) is edited in the panel. */
  reveal(target: string): void;
}

/** Rounds of Jev questions before giving up: the target, then its own. */
const ROUNDS = 3;

/** Asks Jev until Rust has nothing more for it, adding to `answers`. */
async function settle(base: ProjectJson, request: string, answers: Record<string, EditAnswer>): Promise<void> {
  const design = describeDesign(base);
  for (let round = 0; round < ROUNDS; round++) {
    const asks = editQuestions(base, request, answers);
    if (asks.length === 0) return;
    Object.assign(answers, await ask(request, design, asks));
  }
}

/** A request being previewed. */
interface Pending {
  request: string;
  /** The project the request was read against. */
  base: ProjectJson;
  answers: Record<string, EditAnswer>;
  reading: EditReading;
}

const EXAMPLES = ["make it 20' wide", "add a window on the left wall", "a 4 in 12 roof", "lose the porch", "beef up the rafters to 2x8"];

export function mountCommand(root: HTMLElement, ctx: CommandContext): { focus(): void } {
  root.innerHTML = `
    <form class="command-bar" autocomplete="off">
      <span class="command-icon" aria-hidden="true"></span>
      <input type="text" aria-label="Describe a change" spellcheck="false" enterkeyhint="go" />
      <kbd class="command-key" aria-hidden="true"></kbd>
      <button type="submit" class="button">Preview</button>
    </form>
    <div class="proposal" role="region" aria-label="Proposed change" aria-live="polite" hidden></div>
  `;
  const form = root.querySelector<HTMLFormElement>("form")!;
  const input = root.querySelector<HTMLInputElement>("input")!;
  const submit = root.querySelector<HTMLButtonElement>('button[type="submit"]')!;
  const card = root.querySelector<HTMLDivElement>(".proposal")!;
  root.querySelector(".command-key")!.textContent = /Mac|iPhone|iPad/.test(navigator.platform) ? "⌘K" : "Ctrl K";
  input.placeholder = `Describe a change, like "${EXAMPLES[0]}"`;

  let pending: Pending | null = null;
  /** The project before the last change made here, for Undo, and the project as it was made (JSON). */
  let undo: { before: ProjectJson; after: string; summary: string } | null = null;
  /** Bumped on each new request, so a slow Jev call for an old one is dropped. */
  let generation = 0;

  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    const request = input.value.trim();
    if (!request) return;
    const mine = ++generation;
    const base = structuredClone(ctx.project);
    showBusy(request);
    submit.disabled = true;
    try {
      const answers: Record<string, EditAnswer> = {};
      await settle(base, request, answers);
      if (mine !== generation) return;
      pending = { request, base, answers, reading: readEdit(base, request, answers) };
      render();
    } catch (err) {
      if (mine !== generation) return;
      pending = null;
      showError(err instanceof Error ? err.message : String(err));
    } finally {
      if (mine === generation) submit.disabled = false;
    }
  });

  input.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      if (!card.hidden) close();
      else input.blur();
    }
  });
  document.addEventListener("keydown", (e) => {
    if (e.key.toLowerCase() === "k" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      input.focus();
      input.select();
    }
  });

  card.addEventListener("click", (e) => {
    const target = (e.target as Element).closest<HTMLElement>("[data-do]");
    if (!target) return;
    switch (target.dataset.do) {
      case "answer":
        void answer(target.dataset.id!, target.dataset.pick!);
        break;
      case "reveal":
        ctx.reveal(target.dataset.target!);
        break;
      case "apply":
        apply();
        break;
      case "cancel":
        close();
        break;
      case "undo":
        revert();
        break;
      case "example":
        input.value = target.dataset.text!;
        form.requestSubmit();
        break;
    }
  });

  async function answer(id: string, pick: string): Promise<void> {
    if (!pending) return;
    const now = pending;
    const mine = ++generation;
    now.answers[id] = { pick, confidence: 1, by_user: true };
    if (editQuestions(now.base, now.request, now.answers).length > 0) {
      showBusy(now.request);
      try {
        await settle(now.base, now.request, now.answers);
      } catch (err) {
        if (mine === generation) showError(err instanceof Error ? err.message : String(err));
        return;
      }
      if (mine !== generation) return;
    }
    now.reading = readEdit(now.base, now.request, now.answers);
    pending = now;
    render();
  }

  function apply(): void {
    const next = pending?.reading.project;
    if (!pending || !next || pending.reading.problems.length > 0) return;
    // An edit in the panel since the preview would be lost; read it again.
    if (JSON.stringify(pending.base) !== JSON.stringify(ctx.project)) {
      showError("The design changed since this preview. Press Preview to read the request again.");
      pending = null;
      return;
    }
    const changes = pending.reading.steps.flatMap((s) => s.changes);
    const before = pending.base;
    pending = null;
    ctx.replace(structuredClone(next));
    undo = { before, after: JSON.stringify(ctx.project), summary: changes.join(". ") };
    input.value = "";
    showDone();
  }

  function revert(): void {
    if (!undo) return;
    if (undo.after !== JSON.stringify(ctx.project)) {
      // Edited since; undoing would throw that away.
      showError("The design has changed since then, so this can't be undone here.");
      undo = null;
      return;
    }
    ctx.replace(structuredClone(undo.before));
    undo = null;
    close();
  }

  function close(): void {
    generation++;
    pending = null;
    submit.disabled = false;
    card.hidden = true;
    card.replaceChildren();
  }

  function showBusy(request: string): void {
    card.innerHTML = `<p class="proposal-busy"><span class="spinner" aria-hidden="true"></span>Reading "${esc(request)}"…</p>`;
    card.hidden = false;
  }

  function showError(text: string): void {
    card.innerHTML = `
      <p class="proposal-error">${esc(text)}</p>
      <footer><button type="button" class="button" data-do="cancel">Close</button></footer>`;
    card.hidden = false;
  }

  function showDone(): void {
    if (!undo) return;
    card.innerHTML = `
      <p class="proposal-done"><span class="check" aria-hidden="true"></span>${esc(undo.summary)}.</p>
      <footer>
        <button type="button" class="button" data-do="undo">Undo</button>
        <button type="button" class="button" data-do="cancel">Done</button>
      </footer>`;
    card.hidden = false;
  }

  function render(): void {
    if (!pending) return;
    const { reading, base } = pending;
    const steps = reading.steps.map((s) => {
      const changes = s.changes.map((c) => `<li class="change">${esc(c)}</li>`).join("");
      const assumed = s.assumed.map((a) => `<li class="assumed">${esc(a)}</li>`).join("");
      const reveal = s.problem && s.target ? ` <button type="button" class="link-button" data-do="reveal" data-target="${esc(s.target)}">Show it in the panel</button>` : "";
      const problem = s.problem ? `<p class="step-problem">${esc(s.problem)}${reveal}</p>` : "";
      const q = s.question;
      const question = q
        ? `<div class="step-question">
            <p><strong>${esc(q.prompt)}</strong> <span>${esc(reasonText(q.reason))}</span></p>
            <div class="options">${q.options
              .map(
                (o) =>
                  `<button type="button" class="option" data-do="answer" data-id="${esc(q.id)}" data-pick="${esc(o.label)}">${esc(o.description)}</button>`,
              )
              .join("")}</div>
          </div>`
        : "";
      const list = changes || assumed ? `<ul>${changes}${assumed}</ul>` : "";
      return `<section class="step"><q>${esc(s.text)}</q>${list}${question}${problem}</section>`;
    });

    let footer = `<button type="button" class="button" data-do="cancel">Cancel</button>`;
    let effect = "";
    if (reading.project) {
      if (reading.problems.length > 0) {
        effect = `<div class="effect bad"><strong>This won't frame</strong><ul>${reading.problems
          .map((p) => `<li>${esc(capitalize(p))}</li>`)
          .join("")}</ul></div>`;
      } else {
        effect = `<div class="effect">${costLine(base, reading.project)}</div>`;
        footer += `<button type="button" class="button primary" data-do="apply">Make ${countChanges(reading)}</button>`;
      }
    }
    card.innerHTML = `${steps.join("")}${effect}<footer>${footer}</footer>`;
    card.hidden = false;
    card.querySelector<HTMLButtonElement>('[data-do="apply"], .option')?.focus({ preventScroll: true });
  }

  // The first time, show what kinds of things it understands.
  input.addEventListener("focus", () => {
    if (!card.hidden || input.value) return;
    card.innerHTML = `
      <p class="proposal-hint">Say what to change in your own words. You'll see the changes before anything is made.</p>
      <div class="options">${EXAMPLES.map((t) => `<button type="button" class="option" data-do="example" data-text="${esc(t)}">${esc(t)}</button>`).join("")}</div>`;
    card.hidden = false;
  });
  input.addEventListener("input", () => {
    if (card.querySelector(".proposal-hint")) close();
  });
  input.addEventListener("blur", () => {
    // Let a click on an example land first.
    setTimeout(() => {
      if (card.querySelector(".proposal-hint") && !root.contains(document.activeElement)) close();
    }, 150);
  });

  return {
    focus() {
      input.focus();
    },
  };
}

function countChanges(reading: EditReading): string {
  const n = reading.steps.reduce((sum, s) => sum + s.changes.length, 0);
  return n === 1 ? "this change" : `these ${n} changes`;
}

/** Boards, sheets and cost before and after. */
function costLine(before: ProjectJson, after: ProjectJson): string {
  try {
    const a = cutList(before).summary;
    const b = cutList(normalize(structuredClone(after))).summary;
    const parts = [delta("boards", a.boards, b.boards), delta("sheets", a.sheets, b.sheets)];
    if (a.total_cents !== null && b.total_cents !== null) {
      const d = b.total_cents - a.total_cents;
      parts.push(`${dollars(b.total_cents)} <span class="diff">${d === 0 ? "same cost" : `${d > 0 ? "+" : "−"}${dollars(Math.abs(d))}`}</span>`);
    }
    return `<strong>It frames.</strong> ${parts.join(" · ")}`;
  } catch {
    return "<strong>It frames.</strong>";
  }
}

function delta(what: string, a: number, b: number): string {
  const d = b - a;
  return `${b} ${what}${d === 0 ? "" : ` <span class="diff">${d > 0 ? "+" : "−"}${Math.abs(d)}</span>`}`;
}

/** The reason in the user's terms. */
function reasonText(reason: string): string {
  if (reason === "Not answered yet") return "";
  if (reason.startsWith("Best guess")) return "Not sure from the request.";
  return reason.endsWith(".") ? reason : `${reason}.`;
}

function capitalize(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}

function esc(text: string): string {
  return text.replace(/[&<>"]/g, (c) => `&${{ "&": "amp", "<": "lt", ">": "gt", '"': "quot" }[c]};`);
}
