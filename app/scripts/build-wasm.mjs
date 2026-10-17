// Compiles the Rust core to WebAssembly and generates its JS bindings into
// src/wasm. Needs the wasm32-unknown-unknown target and wasm-bindgen-cli at
// the version pinned in crates/sticks-wasm/Cargo.toml.
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const run = (cmd, args) => execFileSync(cmd, args, { cwd: root, stdio: "inherit" });

run("cargo", ["build", "-p", "sticks-wasm", "--target", "wasm32-unknown-unknown", "--release"]);
run("wasm-bindgen", [
  "--target", "web",
  "--out-dir", "app/src/wasm",
  "target/wasm32-unknown-unknown/release/sticks_wasm.wasm",
]);
