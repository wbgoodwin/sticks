// Picks who answers a request's multiple-choice questions: Jev, or the model
// on this machine. Both pick from the same listed options with a confidence,
// so the rest of the command bar doesn't know which answered.
//
// By default Jev answers under the dev server or when signed in to a Sticks
// account, and the local model stands in when Jev can't be reached (no key,
// no network) or there's neither, as in a bundled desktop build signed out.
// VITE_STICKS_ANSWERER=jev or =local in app/.env.local forces one.

import { invoke, isTauri } from "@tauri-apps/api/core";
import type { EditAnswer, EditAsk, Json } from "./core";
import { signedIn } from "./account";
import { askJev } from "./jev";

type Answers = Record<string, EditAnswer>;
type Design = { [key: string]: Json };

const forced = import.meta.env.VITE_STICKS_ANSWERER as "jev" | "local" | undefined;

/** Set once Jev fails, so later requests don't wait on it again. Reload to retry. */
let jevDown = false;

/** The model on this machine, through the desktop app. */
async function askLocal(request: string, design: Design, asks: EditAsk[]): Promise<Answers> {
  if (!isTauri()) throw new Error("The local model runs in the desktop app: npm run tauri dev -- --features local-model.");
  try {
    return await invoke<Answers>("ask_local", { request, design: JSON.stringify(design, null, 2), asks });
  } catch (e) {
    throw new Error(String(e));
  }
}

/** Each question's pick, with how sure it was. */
export async function ask(request: string, design: Design, asks: EditAsk[], change?: string): Promise<Answers> {
  if (forced === "local") return askLocal(request, design, asks);
  const useJev = (import.meta.env.DEV || signedIn()) && !jevDown;
  if (forced === "jev" || (useJev && !isTauri())) return askJev(request, design, asks, change);
  if (useJev) {
    try {
      return await askJev(request, design, asks, change);
    } catch (jev) {
      try {
        const answers = await askLocal(request, design, asks);
        jevDown = true;
        console.info(`Jev unavailable, using the local model: ${(jev as Error).message}`);
        return answers;
      } catch (local) {
        throw new Error(`${(jev as Error).message} The local model couldn't stand in: ${(local as Error).message}`);
      }
    }
  }
  return askLocal(request, design, asks);
}
