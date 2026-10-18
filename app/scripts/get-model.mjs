// Downloads the local model for plain-English edits into models/ at the
// repository root, where the desktop dev build and `npm run probe:local`
// look for it. About 1 GB; run once.
//
//   npm run model:get

import { createWriteStream, existsSync, mkdirSync, renameSync } from "node:fs";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";

// Qwen2.5 1.5B Instruct, 4-bit, Apache 2.0. Keep the name in step with
// DEFAULT_MODEL in crates/sticks-local.
const NAME = "qwen2.5-1.5b-instruct-q4_k_m.gguf";
const SOURCE = `https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/${NAME}`;

const dir = new URL("../../models/", import.meta.url).pathname;
const dest = dir + NAME;
if (existsSync(dest)) {
  console.log(`Already have ${dest}`);
  process.exit(0);
}
mkdirSync(dir, { recursive: true });

let res;
try {
  res = await fetch(SOURCE);
} catch (e) {
  console.error(`Couldn't reach huggingface.co: ${e.cause?.message ?? e.message}`);
  process.exit(1);
}
if (!res.ok || !res.body) {
  console.error(`Download failed: ${res.status} ${res.statusText}`);
  process.exit(1);
}
const total = Number(res.headers.get("content-length")) || 0;
let got = 0;
let shown = -1;
const body = Readable.fromWeb(res.body);
body.on("data", (chunk) => {
  got += chunk.length;
  const pct = total ? Math.floor((got / total) * 100) : -1;
  if (pct !== shown) {
    shown = pct;
    process.stdout.write(`\r${NAME}: ${pct >= 0 ? `${pct}%` : `${(got / 1e6).toFixed(0)} MB`}`);
  }
});
// Written under a temporary name so a broken download isn't picked up.
await pipeline(body, createWriteStream(dest + ".part"));
renameSync(dest + ".part", dest);
console.log(`\nSaved ${dest}`);
