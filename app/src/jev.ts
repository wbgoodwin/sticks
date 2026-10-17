// Asks Jev the questions Rust lists about a request, all in one call.
//
// The page never holds the API key. The dev server forwards /jev to the
// TypeSafe API and adds the key from TYPESAFE_API_KEY (see vite.config.ts),
// so this only works under `npm run dev` or `npm run tauri dev` for now.

import { choice, TypeSafeClient } from "@typesafe-ai/sdk";
import type { EditAnswer, EditAsk } from "./core";

let client: TypeSafeClient | null = null;

/** Jev's pick for each question, with how sure it was. */
export async function askJev(request: string, design: string, asks: EditAsk[]): Promise<Record<string, EditAnswer>> {
  if (!import.meta.env.DEV) {
    throw new Error("Plain-English changes need the dev server for now: run npm run dev or npm run tauri dev.");
  }
  client ??= new TypeSafeClient({
    // The dev server swaps in the real key.
    apiKey: "set-by-the-dev-server",
    baseURL: `${location.origin}/jev`,
    dangerouslyAllowBrowser: true,
  });
  // Each question carries its clause, since one request can hold several.
  const questions = Object.fromEntries(
    asks.map((a) => [
      a.id,
      choice(`${a.prompt} About: "${a.text}"`, Object.fromEntries(a.options.map((o) => [o.label, o.description]))),
    ]),
  );
  let answers;
  try {
    ({ answers } = await client.systemOne({ state: { request, design }, questions }));
  } catch (e) {
    const status = (e as { status?: number }).status;
    if (status === 401) throw new Error("Jev turned down the API key. Set TYPESAFE_API_KEY and restart the dev server.");
    throw new Error(`Couldn't reach Jev: ${e instanceof Error ? e.message : String(e)}`);
  }
  const picked: Record<string, EditAnswer> = {};
  for (const [id, a] of Object.entries(answers)) {
    if (a.type === "choice") picked[id] = { pick: a.choice, confidence: a.confidence };
  }
  return picked;
}
