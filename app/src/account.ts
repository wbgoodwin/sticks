// Signing in to a Sticks account, and talking to the account server
// (sticks-yard) as that account.
//
// Sign-in happens in the browser on Cognito's page (code + PKCE). The
// desktop window waits for the browser to come back to a one-time local
// address; the browser build comes back to /auth/callback on itself. The
// refresh token is kept in local storage, so a sign-in lasts until it is
// revoked or 90 days pass.
//
// Set these in app/.env.local from the sticks-yard deploy's outputs:
//   VITE_YARD_URL=https://api.sticksyard.app   (ApiUrl)
//   VITE_YARD_SIGN_IN=https://sticks-yard.auth.us-east-1.amazoncognito.com  (SignInDomain)
//   VITE_YARD_CLIENT_ID=...                      (ClientId)
// Without them the app has no account features and works as before.

import { invoke, isTauri } from "@tauri-apps/api/core";

const YARD = import.meta.env.VITE_YARD_URL as string | undefined;
const SIGN_IN = import.meta.env.VITE_YARD_SIGN_IN as string | undefined;
const CLIENT_ID = import.meta.env.VITE_YARD_CLIENT_ID as string | undefined;

/** The desktop window's sign-in comes back here; sticks-yard lists it as a callback. */
const DESKTOP_PORT = 47615;
const TOKENS_KEY = "sticks.account";
const PENDING_KEY = "sticks.account.pending";

export const accountsEnabled = Boolean(YARD && SIGN_IN && CLIENT_ID);

interface Tokens {
  access: string;
  refresh: string;
  /** Milliseconds since 1970 when `access` stops working. */
  expires: number;
  email?: string;
}

/** What the server says about the signed-in account (GET /me). */
export interface Me {
  id: string;
  paid: boolean;
  plan: { source: string; status?: string; paidUntil?: string; renews?: boolean; endsOn?: string | null } | null;
  jev: { used: number; included: number; limit: number; extraCents: number; overageCents: number; capCents: number };
}

let tokens: Tokens | null = readTokens();
const listeners = new Set<() => void>();

/** Runs `fn` whenever someone signs in or out. */
export function onAccountChange(fn: () => void): void {
  listeners.add(fn);
}

function changed(): void {
  for (const fn of listeners) fn();
}

export function signedIn(): boolean {
  return tokens !== null;
}

export function email(): string | undefined {
  return tokens?.email;
}

/**
 * Opens Cognito's sign-in page in the browser. In the desktop window this
 * resolves once the browser comes back; in the browser build the page
 * navigates away and finishSignIn() completes it on return.
 */
export async function signIn(): Promise<void> {
  if (!accountsEnabled) throw new Error("This build of Sticks isn't set up for accounts.");
  const verifier = randomString(64);
  const state = randomString(24);
  const redirect = isTauri() ? `http://localhost:${DESKTOP_PORT}/auth/callback` : `${location.origin}/auth/callback`;
  const url = new URL("/oauth2/authorize", SIGN_IN);
  url.search = new URLSearchParams({
    response_type: "code",
    client_id: CLIENT_ID!,
    redirect_uri: redirect,
    scope: "openid email profile",
    state,
    code_challenge: await challenge(verifier),
    code_challenge_method: "S256",
  }).toString();

  if (!isTauri()) {
    sessionStorage.setItem(PENDING_KEY, JSON.stringify({ verifier, state, redirect, back: location.pathname + location.search }));
    location.assign(url.toString());
    return new Promise(() => {});
  }
  // The shell opens the browser and waits for it to come back with the code.
  const query = await invoke<string>("sign_in_in_browser", { url: url.toString(), port: DESKTOP_PORT });
  await finish(new URLSearchParams(query), { verifier, state, redirect });
}

/** In the browser build: completes a sign-in when the page is /auth/callback. */
export async function finishSignIn(): Promise<boolean> {
  if (isTauri() || location.pathname !== "/auth/callback") return false;
  const pending = sessionStorage.getItem(PENDING_KEY);
  sessionStorage.removeItem(PENDING_KEY);
  const params = new URLSearchParams(location.search);
  const back = pending ? (JSON.parse(pending).back as string) : "/";
  history.replaceState(null, "", back);
  if (!pending) return false;
  await finish(params, JSON.parse(pending));
  return true;
}

async function finish(params: URLSearchParams, pending: { verifier: string; state: string; redirect: string }): Promise<void> {
  if (params.get("error")) throw new Error(params.get("error_description") ?? params.get("error")!);
  if (params.get("state") !== pending.state) throw new Error("The sign-in didn't match the one Sticks started. Try again.");
  const code = params.get("code");
  if (!code) throw new Error("The sign-in page didn't send a code back.");
  const body = new URLSearchParams({
    grant_type: "authorization_code",
    client_id: CLIENT_ID!,
    code,
    redirect_uri: pending.redirect,
    code_verifier: pending.verifier,
  });
  const t = await tokenRequest(body);
  setTokens({ access: t.access_token, refresh: t.refresh_token!, expires: Date.now() + t.expires_in * 1000, email: emailFrom(t.id_token) });
}

export async function signOut(): Promise<void> {
  const old = tokens;
  setTokens(null);
  if (!old) return;
  // Best effort: the token stops working on the server too.
  await fetch(new URL("/oauth2/revoke", SIGN_IN), {
    method: "POST",
    headers: { "content-type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({ token: old.refresh, client_id: CLIENT_ID! }),
  }).catch(() => {});
}

/** A current access token, refreshed when it's about to run out. Null when signed out. */
export async function accessToken(): Promise<string | null> {
  if (!tokens) return null;
  if (tokens.expires - Date.now() > 60_000) return tokens.access;
  try {
    const t = await tokenRequest(new URLSearchParams({ grant_type: "refresh_token", client_id: CLIENT_ID!, refresh_token: tokens.refresh }));
    setTokens({ ...tokens, access: t.access_token, expires: Date.now() + t.expires_in * 1000, email: emailFrom(t.id_token) ?? tokens.email });
    return tokens.access;
  } catch (e) {
    // A refresh the server turns down means signing in again.
    if (e instanceof SignInExpired) setTokens(null);
    throw e;
  }
}

class SignInExpired extends Error {}

/** An error from the account server, with its status and message. */
export class YardError extends Error {
  constructor(
    readonly status: number,
    message: string,
    readonly body: Record<string, unknown> = {},
  ) {
    super(message);
  }
}

/** Calls the account server as the signed-in account. */
export async function yard<T>(method: string, path: string, body?: unknown): Promise<T> {
  const token = await accessToken();
  if (!token) throw new YardError(401, "Sign in first.");
  let res: Response;
  try {
    res = await fetch(`${YARD}${path}`, {
      method,
      headers: { authorization: `Bearer ${token}`, ...(body === undefined ? {} : { "content-type": "application/json" }) },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
  } catch {
    throw new YardError(0, "Couldn't reach your Sticks account. Check your internet connection.");
  }
  if (res.status === 204) return undefined as T;
  const json = (await res.json().catch(() => ({}))) as Record<string, unknown>;
  if (res.status === 401) setTokens(null);
  if (!res.ok) throw new YardError(res.status, String(json.error ?? `The account server said ${res.status}.`), json);
  return json as T;
}

/** Where plain-English changes go: the Jev route on the account server. */
export function jevBaseUrl(): string {
  return `${YARD}/jev`;
}

export const me = () => yard<Me>("GET", "/me");

/** Opens a page in the browser: Stripe's checkout or billing pages. */
export async function openInBrowser(url: string): Promise<void> {
  if (!url.startsWith("https://")) throw new Error("Not opening a page that isn't https.");
  if (isTauri()) await invoke("open_in_browser", { url });
  else window.open(url, "_blank", "noopener");
}

/** Starts the paid plan in the browser, ending on `endsOn` (YYYY-MM-DD) when given. */
export async function upgrade(endsOn?: string): Promise<void> {
  const { url } = await yard<{ url: string }>("POST", "/billing/checkout", endsOn ? { endsOn } : {});
  await openInBrowser(url);
}

export async function manageBilling(): Promise<void> {
  const { url } = await yard<{ url: string }>("POST", "/billing/portal");
  await openInBrowser(url);
}

export function setBilling(settings: { overageCapCents?: number; endsOn?: string | null }) {
  return yard<{ overageCapCents: number; renews: boolean | null; endsOn: string | null; paidUntil: string | null }>("PUT", "/billing/settings", settings);
}

async function tokenRequest(body: URLSearchParams) {
  let res: Response;
  try {
    res = await fetch(new URL("/oauth2/token", SIGN_IN), {
      method: "POST",
      headers: { "content-type": "application/x-www-form-urlencoded" },
      body,
    });
  } catch {
    throw new Error("Couldn't reach the sign-in server. Check your internet connection.");
  }
  const json = await res.json().catch(() => ({}));
  if (res.status === 400 && json.error === "invalid_grant") throw new SignInExpired("Your sign-in has expired. Sign in again.");
  if (!res.ok) throw new Error(json.error_description ?? json.error ?? `Sign-in failed (${res.status}).`);
  return json as { access_token: string; refresh_token?: string; id_token?: string; expires_in: number };
}

function setTokens(next: Tokens | null): void {
  const was = tokens !== null;
  tokens = next;
  try {
    if (next) localStorage.setItem(TOKENS_KEY, JSON.stringify(next));
    else localStorage.removeItem(TOKENS_KEY);
  } catch {
    // Without storage the sign-in lasts until the window closes.
  }
  if (was !== (next !== null)) changed();
}

function readTokens(): Tokens | null {
  if (!accountsEnabled) return null;
  try {
    const saved = localStorage.getItem(TOKENS_KEY);
    return saved ? (JSON.parse(saved) as Tokens) : null;
  } catch {
    return null;
  }
}

/** The email in an ID token. It came straight from Cognito over TLS, so it isn't re-checked here. */
function emailFrom(idToken: string | undefined): string | undefined {
  if (!idToken) return undefined;
  try {
    const payload = idToken.split(".")[1]!.replace(/-/g, "+").replace(/_/g, "/");
    return JSON.parse(atob(payload)).email;
  } catch {
    return undefined;
  }
}

function randomString(length: number): string {
  const bytes = crypto.getRandomValues(new Uint8Array(length));
  return base64url(bytes).slice(0, length);
}

async function challenge(verifier: string): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(verifier));
  return base64url(new Uint8Array(digest));
}

function base64url(bytes: Uint8Array): string {
  return btoa(String.fromCharCode(...bytes)).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}
