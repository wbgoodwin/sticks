// Asks Jev the questions Rust lists about a request, all in one call.
//
// The page never holds the API key. Signed in to a Sticks account on the
// paid plan, calls go to the account server, which adds the key and counts
// the change; `change` names the change so its several calls count once.
// Otherwise the dev server forwards /jev to the TypeSafe API with the key
// from TYPESAFE_API_KEY (see vite.config.ts), under `npm run dev` or
// `npm run tauri dev`.

import { choice, TypeSafeClient } from "@typesafe-ai/sdk";
import { accessToken, jevBaseUrl, signedIn, YardError } from "./account";
import type { EditAnswer, EditAsk, Json } from "./core";

let devClient: TypeSafeClient | null = null;

async function client(): Promise<{ jev: TypeSafeClient; account: boolean }> {
  const token = signedIn() ? await accessToken() : null;
  if (token) {
    // The token changes hourly, so the client is made per request.
    return {
      jev: new TypeSafeClient({
        apiKey: token,
        baseURL: jevBaseUrl(),
        dangerouslyAllowBrowser: true,
        compression: false,
        // Our 429s are plan limits, not a busy server: don't hammer them.
        retry: { httpStatuses: new Set([408, 500, 502, 503, 504]) },
      }),
      account: true,
    };
  }
  if (!import.meta.env.DEV) throw new Error("Sign in to your Sticks account to make changes in plain English.");
  devClient ??= new TypeSafeClient({
    // The dev server swaps in the real key.
    apiKey: "set-by-the-dev-server",
    baseURL: `${location.origin}/jev`,
    dangerouslyAllowBrowser: true,
  });
  return { jev: devClient, account: false };
}

/** Jev's pick for each question, with how sure it was. */
export async function askJev(
  request: string,
  design: { [key: string]: Json },
  asks: EditAsk[],
  change: string = crypto.randomUUID(),
): Promise<Record<string, EditAnswer>> {
  const { jev, account } = await client();
  // Each question carries its clause, since one request can hold several.
  const questions = Object.fromEntries(
    asks.map((a) => [
      a.id,
      choice(`${a.prompt} About: "${a.text}"`, Object.fromEntries(a.options.map((o) => [o.label, o.description]))),
    ]),
  );
  let answers;
  try {
    ({ answers } = await jev.systemOne({ state: { request, design }, questions }, { headers: { "x-sticks-change": change } }));
  } catch (e) {
    const status = (e as { status?: number }).status;
    // The account server explains limits and the plan in its own words.
    const said = (e as { body?: { error?: unknown } }).body?.error;
    if (account && status && typeof said === "string") throw new YardError(status, said);
    if (status === 401) throw new Error("Jev turned down the API key. Set TYPESAFE_API_KEY and restart the dev server.");
    throw new Error(`Couldn't reach Jev: ${e instanceof Error ? e.message : String(e)}`);
  }
  const picked: Record<string, EditAnswer> = {};
  for (const [id, a] of Object.entries(answers)) {
    if (a.type === "choice") picked[id] = { pick: a.choice, confidence: a.confidence };
  }
  return picked;
}
