// @ts-check
import { fileURLToPath } from "node:url";

import node from "@astrojs/node";
import preact from "@astrojs/preact";
import { defineConfig } from "astro/config";

// The Astro half of the migration (step A01). Serves BEHIND axum in every deployed shape —
// axum keeps /api, /media, /c4, robots/sitemap, the security headers and compression; this
// app owns pages. In dev it runs on :5373 (the Keycloak dev realm allowlists that origin —
// a silent port bump 403s the silent-SSO iframe, the step-39 scar) and proxies the API to
// the axum server on :8280, exactly as Vite did for the old client.
export default defineConfig({
  output: "server",
  adapter: node({ mode: "standalone" }),
  integrations: [preact()],
  server: { port: 5373 },
  vite: {
    // Pre-bundle Preact + its hooks in ONE dep-optimizer pass, and resolve a SINGLE copy. Without
    // this, a cold `dev` start can optimize `preact` and `preact/hooks` in separate passes with
    // different version hashes — two Preact instances — and the first island to render during that
    // window dies with `Cannot read properties of undefined (reading '__H')` (hooks from one
    // instance, the component from the other). It bit the header auth chip on the very first
    // /edit load; the signed-in e2e reproduced it on every cold run. Dev-only concern (the Rollup
    // build dedupes on its own), but a real dev hits the same cold-start race.
    optimizeDeps: {
      // Pre-bundle the deps reached ONLY through lazy `import()` (Monaco, keycloak-js, the markdown
      // pipeline). Vite's startup scanner only sees statically-imported deps, so these are
      // otherwise discovered on first use mid-session — and Vite responds with a FULL PAGE RELOAD
      // ("new dependencies optimized, reloading"), which drops in-flight island state (it wiped the
      // review dialog the instant the preview first rendered). Listing them here means the whole
      // graph is optimized once at startup and no interaction triggers a reload. Costs a little
      // dev-startup time; buys a reload-free session for everyone, not just the e2e.
      include: [
        "preact",
        "preact/hooks",
        "preact/jsx-runtime",
        "preact/compat",
        "keycloak-js",
        "monaco-editor",
        "unified",
        "remark-parse",
        "remark-gfm",
        "remark-rehype",
        "rehype-slug",
        "rehype-pretty-code",
        "rehype-stringify",
        "shiki",
        "mdast-util-to-hast",
      ],
    },
    resolve: {
      dedupe: ["preact", "preact/hooks"],
      // The viz wasm's bindgen glue imports `@editor/loader` / `@tracer/loader` (the crate's
      // FFI externs, A10) — the same specifiers the old client's Vite resolves, pointed at the
      // same single-sourced islands.
      alias: {
        "@editor": fileURLToPath(new URL("./src/lib/islands/editor", import.meta.url)),
        "@tracer": fileURLToPath(new URL("./src/lib/islands/tracer", import.meta.url)),
      },
    },
    server: {
      strictPort: true,
      proxy: {
        "/api": "http://localhost:8280",
        "/media": "http://localhost:8280",
        "/c4": "http://localhost:8280",
      },
      fs: {
        // The viz wasm pkg lives under src/, but Cargo.lock (read by the wasm build check)
        // and the repo root are outside the app dir.
        allow: [".."],
      },
    },
  },
});
