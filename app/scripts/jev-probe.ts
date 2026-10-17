// Runs sample requests the way the app does: Rust asks Jev what in the
// design each part of a request changes, then that thing's own questions,
// and turns the confident answers into edits. Whatever Jev wasn't sure of,
// or the request doesn't say, is left for you.
//
//   cargo build -p sticks-cli
//   TYPESAFE_API_KEY=... node scripts/jev-probe.ts

import { execFileSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { choice, TypeSafeClient } from "@typesafe-ai/sdk";

const SHED = new URL("../../examples/shed.toml", import.meta.url).pathname;
const STICKS = new URL("../../target/debug/sticks", import.meta.url).pathname;

const REQUESTS = [
  "make it 20' wide and add a window on the left wall",
  "beef up the rafters to 2x8",
  "make the roof a gable",
  "let some light into the right wall",
  "I want a way in on the back",
  "get rid of the hole for the mini split",
  "make it bigger by 2'",
  "add a window",
  "the roof should shed rain better",
  "paint it red",
  "put a 36x48 window in the right wall 2' from the left",
  "make the porch 8' deep",
  "please add a window on each side of the door",
  "make the door 60 inches by 80 inches",
  "center the front door",
  "add two windows to the back wall",
];

// What `sticks edit --questions` prints.
interface Ask {
  type: "choice";
  id: string;
  text: string;
  prompt: string;
  options: { label: string; description: string }[];
}

// What `sticks edit --answers` reads.
interface Answer {
  pick: string;
  confidence: number;
}

// What `sticks edit --json` prints.
interface Reading {
  steps: {
    text: string;
    target?: string;
    edits: unknown[];
    assumed?: string[];
    question?: { id: string; prompt: string; reason: string };
    problem?: string;
  }[];
  preview: { problems: string[] } | null;
}

const run = (args: string[]) => execFileSync(STICKS, ["edit", SHED, ...args], { encoding: "utf8" });
const design = execFileSync(STICKS, ["check", SHED], { encoding: "utf8" });
const dir = mkdtempSync(join(tmpdir(), "jev-probe-"));

const key = process.env.TYPESAFE_API_KEY;
if (!key) {
  console.log("Set TYPESAFE_API_KEY to run the probe. The questions for the first request:\n");
  console.log(run([REQUESTS[0], "--questions"]));
  process.exit(0);
}
const client = new TypeSafeClient();

const file = join(dir, "answers.json");
for (const request of REQUESTS) {
  // Round one asks what each part changes; round two, that thing's questions.
  const picked: Record<string, Answer> = {};
  for (let round = 0; round < 3; round++) {
    writeFileSync(file, JSON.stringify(picked));
    const asks = JSON.parse(run([request, "--answers", file, "--questions"])) as Ask[];
    if (asks.length === 0) break;
    // Each question carries its clause, since one request can hold several.
    const questions = Object.fromEntries(
      asks.map((a) => [
        a.id,
        choice(`${a.prompt} About: "${a.text}"`, Object.fromEntries(a.options.map((o) => [o.label, o.description]))),
      ]),
    );
    const { answers } = await client.systemOne({ state: { request, design }, questions });
    for (const [id, a] of Object.entries(answers)) {
      if (a.type === "choice") picked[id] = { pick: a.choice, confidence: a.confidence };
    }
  }
  writeFileSync(file, JSON.stringify(picked));
  const reading = JSON.parse(run([request, "--answers", file, "--json"])) as Reading;

  const asked = reading.steps.some((s) => s.question);
  console.log(`\n"${request}"  [${asked ? "asks you" : "settled"}]`);
  for (const s of reading.steps) {
    const i = reading.steps.indexOf(s);
    const pct = (a: Answer) => `${a.pick} ${Math.round(a.confidence * 100)}%`;
    const target = picked[`${i}.target`];
    if (target) console.log(`  "${s.text}": ${pct(target)}`);
    const shown = Object.keys(picked)
      .filter((id) => id.startsWith(`${i}.`) && id !== `${i}.target` && picked[id].pick !== "unsaid")
      .map((id) => `${id.slice(id.indexOf(".") + 1)} ${pct(picked[id])}`);
    if (shown.length) console.log(`    picks: ${shown.join(", ")}`);
    for (const e of s.edits) console.log(`    edit: ${JSON.stringify(e)}`);
    for (const a of s.assumed ?? []) console.log(`    assumed: ${a}`);
    if (s.question) console.log(`    ask: ${s.question.prompt} (${s.question.reason})`);
    if (s.problem) console.log(`    needs: ${s.problem}`);
  }
  for (const p of reading.preview?.problems ?? []) console.log(`  won't frame: ${p}`);
}
