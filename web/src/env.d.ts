/// <reference path="../.astro/types.d.ts" />
/// <reference types="astro/client" />

// The custom env vars this app reads (astro's IntelliSense convention:
// https://docs.astro.build/en/guides/environment-variables/#intellisense). Both are optional —
// every reader falls back to a default, never throws on an unset var.
interface ImportMetaEnv {
  /** The axum origin for SSR fetches (client.ts's `apiBase`). Unset → http://localhost:8280. */
  readonly SYNAPSE_API_URL?: string;
  /** The public origin for canonical/OG URLs (layouts/Base.astro). Unset → the prod origin. */
  readonly SYNAPSE_SITE_URL?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}

// Monaco's deep ESM entry ships no types — the moved island imports it untyped by design.
declare module "monaco-editor/esm/vs/editor/edcore.main";
