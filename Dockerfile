# ──────────────────────────────────────────────────────────────────────────────
# THE PRODUCTION IMAGE (step 35; oracle: step-34/ADR-S033) — two stages,
# CONTENT-FREE: SYNAPSE_ROOT points at the git-sync sidecar's volume, so prose
# publishing is a `git push` to synapse-content and the image rebuilds only
# when CODE changes. The bundle-budget gate runs INSIDE the build — an
# over-budget bundle fails the image, not the reader's first paint.
# ──────────────────────────────────────────────────────────────────────────────

FROM rust:1-bookworm AS builder

# Node 22 for the Vite build (matches the dev toolchain).
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y --no-install-recommends nodejs \
    && rm -rf /var/lib/apt/lists/*

# binaryen from upstream, PINNED: Debian bookworm ships binaryen 108 (2022), whose
# `wasm-opt -Oz` leaves ~270 KiB gz on the table vs a current release — enough to
# blow the 700 KiB budget gate below. (Found by the gate itself, working as built.)
ARG BINARYEN_VERSION=123
RUN arch="$(uname -m)" \
    && curl -fsSL "https://github.com/WebAssembly/binaryen/releases/download/version_${BINARYEN_VERSION}/binaryen-version_${BINARYEN_VERSION}-${arch}-linux.tar.gz" \
       | tar -xz -C /usr/local --strip-components=1 \
    && wasm-opt --version

RUN rustup target add wasm32-unknown-unknown
# Pinned to Cargo.lock's wasm-bindgen (build-wasm.sh refuses a mismatch).
RUN cargo install wasm-bindgen-cli --version 0.2.126 --locked

WORKDIR /build
COPY . .

# The server binary (release), then the client: `npm run build` chains the
# release wasm pipeline (cargo → wasm-bindgen → wasm-opt -Oz) and `vite build`.
RUN cargo build --release -p synapse-server
RUN cd client && npm ci --no-audit --no-fund && npm run build

# The gate: gzipped critical path (entry + modulepreloads + app wasm) ≤ 700 KiB.
RUN bash dev-tools/check-bundle-budget.sh client/dist

# ──────────────────────────────────────────────────────────────────────────────

FROM debian:bookworm-slim AS runtime

# CA roots for the outbound reqwest clients (Keycloak · go-judge · Ollama).
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /build/target/release/synapse-server /app/synapse-server
COPY --from=builder /build/client/dist /app/static

# The pod runs as a NON-ROOT uid (65532, matching the deployment's runAsUser);
# a+rX makes the dist world-readable/traversable and keeps execute only where
# it already was (the binary).
RUN chmod -R a+rX /app

ENV STATIC_ROOT=/app/static \
    SYNAPSE_ROOT=/content \
    SYNAPSE_AUTO_RELOAD=false \
    SYNAPSE_PORT=8080

EXPOSE 8080
USER 65532:65532
CMD ["/app/synapse-server"]
