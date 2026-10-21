// Builds the desktop app from this copy of the source and installs it.
//
// On a Mac this builds Sticks.app and copies it to /Applications, replacing
// an older one. Elsewhere it builds the platform's packages and says where
// they are. Run it again after `git pull` to update.
import { execFileSync } from "node:child_process";
import { existsSync, rmSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join, resolve } from "node:path";

const app = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const run = (cmd, args) => execFileSync(cmd, args, { cwd: app, stdio: "inherit" });
const bundle = join(app, "src-tauri/target/release/bundle");

run("npm", ["ci"]);

if (process.platform !== "darwin") {
  run("npx", ["tauri", "build"]);
  console.log(`\nBuilt. The installers are in ${bundle}`);
  process.exit(0);
}

// Just the .app: a .dmg isn't needed to install your own build.
run("npx", ["tauri", "build", "--bundles", "app"]);
const built = join(bundle, "macos/Sticks.app");
const target = "/Applications/Sticks.app";
if (existsSync(target)) rmSync(target, { recursive: true, force: true });
// ditto keeps the bundle's symlinks and code signature intact.
run("ditto", [built, target]);
console.log(`\nInstalled ${target}. Open it from Launchpad or Spotlight.`);
