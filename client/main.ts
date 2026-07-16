// ─────────────────────────────────────────────────────────────────────────────
// ENTRY — boots the wasm app
// Vite bundles the wasm-bindgen glue (resolving the @alias island imports it
// carries); the explicit URL keeps the .wasm asset itself under Vite's control.
// ─────────────────────────────────────────────────────────────────────────────

import "./styles/markdown.css";
import "./styles/reader.css";
import "./styles/runnable.css";
import "./styles/blog.css";
import "./styles/search.css";
import "./styles/account.css";
import "./styles/coach.css";
import init from "./pkg/synapse_client.js";

await init({
  module_or_path: new URL("./pkg/synapse_client_bg.wasm", import.meta.url),
});
