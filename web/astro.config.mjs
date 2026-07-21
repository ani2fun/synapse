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
    resolve: {
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
        // The stylesheets stay in client/styles until the old client is deleted (A14) — both
        // apps import the same files, so there is no drift window.
        allow: [".."],
      },
    },
  },
});
