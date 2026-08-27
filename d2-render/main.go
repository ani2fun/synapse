package main

// ── d2-render ────────────────────────────────────────────────────────────────
// Draws this catalog's ```d2 fences, on demand, beside the app.
//
// It exists because rendering d2 in the page tier is not affordable and rendering it in the reader
// is not free. The wasm build under Node peaks at ~5.2 GB of RSS on a 23-diagram lesson against a
// 256Mi container (ADR-RS007); the same work through the native engine here peaks at ~172 MB.
// Sending the engine to the reader instead costs ~5.9 MB gz before the first figure appears.
//
// So: one long-lived process, its own container and its own memory limit, and a content-addressed
// cache in front of it. A figure is drawn once ever, on the first request that wants it, and
// served from disk after that. Nothing is generated at publish time and nothing is committed.
//
// Every failure is a 4xx/5xx with a plain-text reason. The caller treats all of them as "not
// drawn" and falls back to the client renderer, which is the same floor a d2 miss has always
// landed on — the page renders either way.

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
	"strings"
	"time"
)

// A malformed diagram must not hold a connection open; the app's own budget is shorter still.
const renderTimeout = 20 * time.Second

// Sources are prose-sized. The cap is here so a stray upload cannot become the process's memory
// profile, not because any real fence approaches it.
const maxSourceBytes = 1 << 20

var store *diskCache

func logf(format string, args ...any) { log.Printf(format, args...) }

func main() {
	addr := env("D2_RENDER_ADDR", ":8390")
	dir := env("D2_RENDER_CACHE", "/var/cache/d2-render")

	cache, err := newDiskCache(dir)
	if err != nil {
		log.Fatalf("d2-render: %v", err)
	}
	store = cache

	mux := http.NewServeMux()
	mux.HandleFunc("GET /healthz", func(w http.ResponseWriter, _ *http.Request) {
		w.WriteHeader(http.StatusOK)
		_, _ = io.WriteString(w, "ok\n")
	})
	mux.HandleFunc("POST /render", handleRender)
	mux.HandleFunc("POST /boards", handleBoards)
	mux.HandleFunc("GET /board/{hash}/{slug}", handleBoard)

	server := &http.Server{
		Addr:              addr,
		Handler:           mux,
		ReadHeaderTimeout: 5 * time.Second,
	}
	logf("d2-render: listening on %s, cache %s", addr, dir)
	if err := server.ListenAndServe(); err != nil && !errors.Is(err, http.ErrServerClosed) {
		log.Fatalf("d2-render: %v", err)
	}
}

func env(key, fallback string) string {
	if value := os.Getenv(key); value != "" {
		return value
	}
	return fallback
}

// ── REQUESTS ─────────────────────────────────────────────────────────────────

type renderRequest struct {
	Source string `json:"source"`
	Meta   string `json:"meta"`
}

func readRequest(w http.ResponseWriter, r *http.Request) (*renderRequest, bool) {
	body, err := io.ReadAll(io.LimitReader(r.Body, maxSourceBytes+1))
	if err != nil {
		fail(w, http.StatusBadRequest, "unreadable body")
		return nil, false
	}
	if len(body) > maxSourceBytes {
		fail(w, http.StatusRequestEntityTooLarge, "source too large")
		return nil, false
	}
	var req renderRequest
	if err := json.Unmarshal(body, &req); err != nil {
		fail(w, http.StatusBadRequest, "malformed JSON")
		return nil, false
	}
	if strings.TrimSpace(req.Source) == "" {
		fail(w, http.StatusBadRequest, "empty source")
		return nil, false
	}
	return &req, true
}

func fail(w http.ResponseWriter, code int, reason string) {
	w.Header().Set("Content-Type", "text/plain; charset=utf-8")
	w.WriteHeader(code)
	_, _ = io.WriteString(w, reason+"\n")
}

func writeSVG(w http.ResponseWriter, svg []byte) {
	w.Header().Set("Content-Type", "image/svg+xml")
	// Immutable by construction: the key is the content. The caller may hold it forever.
	w.Header().Set("Cache-Control", "public, max-age=31536000, immutable")
	_, _ = w.Write(svg)
}

// ── ONE FIGURE ───────────────────────────────────────────────────────────────

// handleRender draws an ordinary ```d2 fence.
//
// The salt is derived here rather than taken from the caller, so one diagram has one cache entry
// however many times a document repeats it. A repeat is re-salted by the reader, which is what it
// already did when these files were committed artifacts.
func handleRender(w http.ResponseWriter, r *http.Request) {
	req, ok := readRequest(w, r)
	if !ok {
		return
	}
	hash := fnv1a(req.Source)
	ctx, cancel := context.WithTimeout(r.Context(), renderTimeout)
	defer cancel()

	svg, err := store.do(hash+".svg", func() ([]byte, error) {
		return renderSource(ctx, req.Source, "d2-"+hash)
	})
	if err != nil {
		logf("render %s: %v", hash, err)
		fail(w, http.StatusUnprocessableEntity, describe(err))
		return
	}
	writeSVG(w, svg)
}

// ── ONE WALKTHROUGH ──────────────────────────────────────────────────────────

type boardsResponse struct {
	Manifest BoardManifest `json:"manifest"`
	RootSVG  string        `json:"rootSvg"`
}

// handleBoards compiles a ```d2 boards fence and draws EVERY board, returning the manifest and the
// root.
//
// Every board, because the compile is the expensive half and the alternative is recompiling the
// whole source each time the reader clicks into one. Only the root comes back in the response: the
// others are a click away that many readers never take, and inlining them would put bytes nobody
// reads into every page. They are already on disk when that click comes.
func handleBoards(w http.ResponseWriter, r *http.Request) {
	req, ok := readRequest(w, r)
	if !ok {
		return
	}
	hash := fnv1a(req.Source)
	ctx, cancel := context.WithTimeout(r.Context(), renderTimeout)
	defer cancel()

	raw, err := store.do(hash+".boards.json", func() ([]byte, error) {
		return drawWalkthrough(ctx, req.Source, req.Meta, hash)
	})
	if err != nil {
		logf("boards %s: %v", hash, err)
		fail(w, http.StatusUnprocessableEntity, describe(err))
		return
	}

	var manifest BoardManifest
	if err := json.Unmarshal(raw, &manifest); err != nil {
		fail(w, http.StatusInternalServerError, "unreadable manifest")
		return
	}
	root, ok := store.get(hash + "." + rootSlugOf(manifest) + ".svg")
	if !ok {
		// The manifest is cached and its boards are not: a volume wiped between the two writes.
		// Say "not drawn" rather than serve a walkthrough whose first board is missing.
		fail(w, http.StatusNotFound, "root board not cached")
		return
	}
	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(boardsResponse{Manifest: manifest, RootSVG: string(root)})
}

func rootSlugOf(manifest BoardManifest) string {
	for _, board := range manifest.Boards {
		if board.ID == manifest.Root {
			return board.Slug
		}
	}
	return ""
}

// drawWalkthrough compiles once, renders every board into the cache, and returns the manifest.
func drawWalkthrough(ctx context.Context, source, meta, hash string) ([]byte, error) {
	diagram, err := compileDiagram(ctx, source)
	if err != nil {
		return nil, err
	}
	boards := walkBoards(diagram, rootTitleOf(meta))
	if len(boards) == 0 {
		return nil, errors.New("no renderable boards")
	}
	for _, board := range boards {
		svg, err := renderDiagram(board.node, saltForBoard(hash, board.ID))
		if err != nil {
			return nil, fmt.Errorf("board %s: %w", board.ID, err)
		}
		if err := store.put(hash+"."+board.Slug+".svg", svg); err != nil {
			return nil, err
		}
	}
	return json.Marshal(manifestFor(source, boards))
}

// handleBoard serves one already-drawn board. A miss is a 404 rather than a render: the manifest
// naming this slug came from a `/boards` call that drew it, so a miss means the cache was cleared
// underneath the reader and the honest answer is to let them fall back.
func handleBoard(w http.ResponseWriter, r *http.Request) {
	hash := r.PathValue("hash")
	slug := strings.TrimSuffix(r.PathValue("slug"), ".svg")
	svg, ok := store.get(hash + "." + slug + ".svg")
	if !ok {
		fail(w, http.StatusNotFound, "not drawn")
		return
	}
	writeSVG(w, svg)
}

// describe trims an engine error to its first line. d2 reports a parse failure with a position and
// then the offending source, which belongs in the log rather than in an HTTP status line.
func describe(err error) string {
	line, _, _ := strings.Cut(err.Error(), "\n")
	if len(line) > 300 {
		line = line[:300]
	}
	return line
}
