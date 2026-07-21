# ──────────────────────────────────────────────────────────────────────────────
# THE PRODUCTION IMAGE — two stages, CONTENT-FREE: SYNAPSE_ROOT points at the
# git-sync sidecar's volume, so prose publishing is a `git push` to the content
# repo and the image rebuilds only when CODE changes. Runtime is TWO processes
# (start.sh): the axum server fronting the Astro SSR sidecar; either death
# kills the container. The per-page JS budget gates in CI's e2e job, which has
# the live stack this build stage does not.
# ──────────────────────────────────────────────────────────────────────────────

FROM rust:1-bookworm AS builder

# Node 22 for the Astro build (matches the dev toolchain).
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y --no-install-recommends nodejs \
    && rm -rf /var/lib/apt/lists/*

# binaryen from upstream, PINNED: Debian bookworm ships binaryen 108 (2022), which
# both MISCOMPILES modules from a current toolchain and leaves ~270 KiB gz on the
# table. CI reads the version from this line so the two can never drift.
ARG BINARYEN_VERSION=123
RUN arch="$(uname -m)" \
    && curl -fsSL "https://github.com/WebAssembly/binaryen/releases/download/version_${BINARYEN_VERSION}/binaryen-version_${BINARYEN_VERSION}-${arch}-linux.tar.gz" \
       | tar -xz -C /usr/local --strip-components=1 \
    && wasm-opt --version

RUN rustup target add wasm32-unknown-unknown
# Pinned to Cargo.lock's wasm-bindgen (build-viz-wasm.sh refuses a mismatch).
RUN cargo install wasm-bindgen-cli --version 0.2.126 --locked

# The shipped version: the footer names the build the reader is actually running.
# `.dockerignore` excludes `.git`, so the arg is the only path that works here, and the
# default makes an un-argued build say so rather than lie.
ARG SYNAPSE_VERSION=unknown
ENV SYNAPSE_VERSION=$SYNAPSE_VERSION

WORKDIR /build
COPY . .

RUN cargo build --release -p synapse-server

# The viz wasm bundle is gitignored build output the web build imports, so it comes
# first (release profile: the artifact CI's e2e budget caps). binaryen + wasm-bindgen
# are already installed above.
RUN bash dev-tools/build-viz-wasm.sh release

# `npm ci` (full — the astro build needs its devDeps), the SSR + client build, then
# prune to a PROD node_modules IN PLACE. The @astrojs/node standalone server is NOT
# self-contained: its bundle externalises framework deps (preact, unified, shiki, …)
# as bare specifiers that only resolve against node_modules — VERIFIED empirically
# (entry.mjs crashes ERR_MODULE_NOT_FOUND without it). We prune-and-copy rather than
# re-`npm ci` in the runtime so that stage stays offline and the lockfile install is
# the single source of truth.
RUN cd web && npm ci --no-audit --no-fund && npm run build && npm prune --omit=dev

# Drop the three heaviest deps (~238 MB) from the PROD node_modules that ships: monaco-editor,
# mermaid, @terrastruct/d2 are browser-only lazy islands — already bundled into dist/client's
# hashed chunks, never imported at SSR time. The standalone server's externalised imports were
# enumerated (grep of dist/server's bare specifiers) and name NONE of them; the CI lesson-page
# boot check (it exercises shiki/remark) catches a future Astro version that starts
# externalizing one, turning silent bloat into a loud ERR_MODULE_NOT_FOUND.
RUN rm -rf web/node_modules/monaco-editor web/node_modules/mermaid web/node_modules/@terrastruct

# ──────────────────────────────────────────────────────────────────────────────

FROM debian:bookworm-slim AS runtime

# CA roots for the outbound reqwest clients (Keycloak · go-judge · Ollama), then
# Node 22 for the SSR sidecar (nodesource, same channel as the builder). curl is
# nodesource's own prerequisite. bash is already present in bookworm-slim (start.sh
# needs it for `wait -n`).
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y --no-install-recommends nodejs \
    && apt-get purge -y curl && apt-get autoremove -y \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /build/target/release/synapse-server /app/synapse-server

# The Astro app: its SSR dist under /app/web, and the PROD node_modules the
# standalone server resolves its externalised imports against (see the builder note).
# node walks up from /app/web/server/entry.mjs and finds /app/web/node_modules.
COPY --from=builder /build/web/dist /app/web
COPY --from=builder /build/web/node_modules /app/web/node_modules
COPY dev-tools/start.sh /app/start.sh

# The pod runs as a NON-ROOT uid (65532, matching the deployment's runAsUser);
# a+rX makes everything world-readable/traversable and keeps execute only where it
# already was (the binary), then the entrypoint is made explicitly executable — a+rX
# would not add +x to a plain COPY'd file.
RUN chmod -R a+rX /app && chmod 0755 /app/start.sh

# The footer reads this at request time in the sidecar (the builder's copy inlines nothing —
# Vite only statically injects PUBLIC_-prefixed vars).
ARG SYNAPSE_VERSION=unknown
ENV SYNAPSE_VERSION=$SYNAPSE_VERSION

ENV SYNAPSE_ROOT=/content \
    SYNAPSE_AUTO_RELOAD=false \
    SYNAPSE_PORT=8080 \
    SYNAPSE_ASTRO_URL=http://127.0.0.1:4321

EXPOSE 8080
USER 65532:65532
CMD ["/app/start.sh"]
