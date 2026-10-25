/// <reference types="vite/client" />

/** The app's version, from src-tauri/tauri.conf.json (see vite.config.ts). */
declare const __APP_VERSION__: string;

interface ImportMetaEnv {
  /** "on" shows the plain-English command bar, which asks Jev. Off by default. */
  readonly VITE_JEV?: string;
}
