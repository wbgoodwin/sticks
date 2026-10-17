// Runs sample requests through `sticks edit`, sends the questions Rust
// can't settle to Jev, and shows who settled each one: Rust, Jev, or you.
//
//   cargo build -p sticks-cli
//   TYPESAFE_API_KEY=... node scripts/jev-probe.ts
//
// Without a key it prints the questions Jev would get.

import { execFileSync } from "node:child_process";
import { choice, TypeSafeClient } from "@typesafe-ai/sdk";

const SHED = new URL("../../examples/shed.toml", import.meta.url).pathname;
const STICKS = new URL("../../target/debug/sticks", import.meta.url).pathname;
// Below this, the probe asks you instead of taking Jev's pick.
const CONFIDENT = 0.8;

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
];

// What `sticks edit --json` prints.
interface Choice {
  label: string;
  description: string;
}
interface Question {
  id: string;
  text: string;
  prompt: string;
  options: Choice[];
  // Nothing in the request bears on it, so Jev could only guess.
  for_user: boolean;
}
interface Step {
  text: string;
  edits: unknown[];
  assumed?: string[];
  question?: Question;
  problem?: string;
}
interface Reading {
  steps: Step[];
  ready: boolean;
  preview: { problems: string[] } | null;
}

function sticks(request: string, answers: Map<string, string>): Reading {
  const args = ["edit", SHED, request, "--json"];
  for (const [id, label] of answers) args.push("--answer", `${id}=${label}`);
  return JSON.parse(execFileSync(STICKS, args, { encoding: "utf8" })) as Reading;
}

const design = execFileSync(STICKS, ["check", SHED], { encoding: "utf8" });
const client = process.env.TYPESAFE_API_KEY ? new TypeSafeClient() : null;

for (const request of REQUESTS) {
  const answers = new Map<string, string>();
  const byJev: string[] = [];
  const forYou: string[] = [];
  let reading = sticks(request, answers);

  // Each answer can open the next question (intent, then wall, ...).
  for (let round = 0; round < 4; round++) {
    const open = reading.steps.flatMap((s) => (s.question ? [s.question] : []));
    for (const q of open.filter((q) => q.for_user && !forYou.some((f) => f.startsWith(q.id)))) {
      forYou.push(`${q.id} ${q.prompt} (the request doesn't say)`);
    }
    const unasked = open.filter((q) => !forYou.some((f) => f.startsWith(q.id)));
    if (unasked.length === 0) break;
    if (!client) {
      for (const q of unasked) forYou.push(`${q.id} ${q.prompt} [${q.options.map((o) => o.label).join(", ")}]`);
      break;
    }
    // One batched call: every open question about the same request.
    const questions = Object.fromEntries(
      unasked.map((q) => [
        q.id,
        choice(`${q.prompt} The part of the request this is about: "${q.text}"`, Object.fromEntries(q.options.map((o) => [o.label, o.description]))),
      ]),
    );
    const { answers: got } = await client.systemOne({ state: { request, design }, questions });
    for (const q of unasked) {
      const a = got[q.id];
      if (a.type !== "choice") continue;
      const pct = `${Math.round(a.confidence * 100)}%`;
      if (a.confidence >= CONFIDENT) {
        answers.set(q.id, a.choice);
        byJev.push(`${q.id}=${a.choice} (${pct})`);
      } else {
        forYou.push(`${q.id} ${q.prompt} Jev leaned ${a.choice} at ${pct}`);
      }
    }
    reading = sticks(request, answers);
  }

  const settledBy = forYou.length ? "asks you" : byJev.length ? "Rust + Jev" : "Rust";
  console.log(`\n"${request}"  [${settledBy}]`);
  for (const j of byJev) console.log(`  jev: ${j}`);
  for (const f of forYou) console.log(`  ask: ${f}`);
  for (const s of reading.steps) {
    for (const e of s.edits) console.log(`  edit: ${JSON.stringify(e)}`);
    if (s.problem) console.log(`  needs: ${s.problem}`);
  }
  for (const p of reading.preview?.problems ?? []) console.log(`  won't frame: ${p}`);
}
