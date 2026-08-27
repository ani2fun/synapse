# ── d2-render ─────────────────────────────────────────────────────────────────
# Its OWN image, not a third process in the app's. The point of the split is the memory limit: a
# render peaks around 172 MB and the page tier lives in 256Mi, so the renderer needs a budget of
# its own that a spike cannot spend on the app's behalf (ADR-RS009).
#
# ~40 MB against the app image's ~950 MB, because none of that image applies here: no Node, no
# Rust, no content. d2 embeds its own fonts, which is most of the binary and the reason no font
# package is installed.

# bookworm to match the app image's `rust:1-bookworm` builder, and an EXACT patch version for the
# reason rust-toolchain.toml pins Rust exactly: `go test` runs vet, new Go releases add vet checks,
# and a floating toolchain turns CI red on code nobody touched — the build breaking for a reason
# that is not in the diff. It agrees with the `go` directive in go.mod, so a local build and CI
# compile with the same compiler.
#
# Not alpine: this stage is thrown away — the runtime below is distroless/static at ~33 MB — so a
# smaller builder would buy CI pull time and pay for it in musl-vs-glibc differences.
FROM golang:1.26.6-bookworm AS builder
WORKDIR /build

# Dependencies first, so a change to our own source does not re-download the module graph.
COPY go.mod go.sum ./
RUN go mod download

COPY . .
# CGO off: d2 runs ELK on goja, a pure-Go JavaScript runtime, so nothing here needs a libc and the
# result can sit on a scratch image. `-s -w` drops the symbol table and DWARF — some 8 MB of
# debugging information no production stack trace uses.
RUN CGO_ENABLED=0 GOOS=linux go build -trimpath -ldflags="-s -w" -o /out/d2-render . \
 && mkdir -p /out/cache

# ── runtime ───────────────────────────────────────────────────────────────────
# `static` rather than `base`: nothing dynamically links, and a smaller image is a smaller thing to
# audit. `nonroot` is uid 65532.
FROM gcr.io/distroless/static-debian12:nonroot

COPY --from=builder /out/d2-render /d2-render
# Created here and owned by the runtime user, so the container still caches when nothing is mounted
# over it. A pod SHOULD mount a volume here — an emptyDir is enough, since a figure is a pure
# function of its source and a cold cache is slow rather than wrong — and that volume must be
# writable by uid 65532 (`fsGroup: 65532`), or every render is recomputed and the log fills with
# "not stored".
COPY --from=builder --chown=65532:65532 /out/cache /var/cache/d2-render

ENV D2_RENDER_ADDR=":8390" \
    D2_RENDER_CACHE="/var/cache/d2-render"
EXPOSE 8390
USER nonroot
ENTRYPOINT ["/d2-render"]
