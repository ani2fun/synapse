package main

import (
	"context"
	"strings"
	"testing"

	"oss.terrastruct.com/d2/d2target"
)

// As with the hash vectors, these came from the TypeScript (`dev-tools/d2-boards.mjs`). A slug
// that disagrees by one character names a board the viewer never asks for — silently, because the
// miss falls back to the client renderer rather than erroring.
func TestBoardSlugMatchesTypeScript(t *testing.T) {
	for _, tc := range []struct{ id, want string }{
		{"root", "root"},
		{"root.layers.container", "container"},
		{"root.layers.a.steps.b", "a-b"},
		{"root.layers.Redirect_Handler", "redirect_handler"},
		{"root.layers.a b c", "a-b-c"},
		{"root.layers.***", "board"}, // punctuation-only: a name, never an empty path segment
		{"root.scenarios.x.scenarios.y", "x-y"},
	} {
		if got := boardSlug(tc.id); got != tc.want {
			t.Errorf("boardSlug(%q) = %q, want %q", tc.id, got, tc.want)
		}
	}
}

func TestSaltForBoardMatchesTypeScript(t *testing.T) {
	if got := saltForBoard("deadbeef", "root.layers.container"); got != "d2-deadbeef-container" {
		t.Errorf("saltForBoard = %q", got)
	}
}

// Only `root=` is parsed here — see boards.go. The vector came from the TypeScript, which is what
// makes a quoted-vs-bare disagreement visible rather than a matter of taste.
func TestRootTitleMatchesTypeScript(t *testing.T) {
	for _, tc := range []struct{ meta, root string }{
		{`boards name="url-shortener" root="Context"`, "Context"},
		{`boards root=Context`, "Context"},
		{`boards`, ""},
		{`boards name="x"`, ""},
	} {
		if got := rootTitleOf(tc.meta); got != tc.root {
			t.Errorf("rootTitleOf(%q) = %q, want %q", tc.meta, got, tc.root)
		}
	}
}

func TestTitleCase(t *testing.T) {
	for _, tc := range []struct{ in, want string }{
		{"redirect_handler", "Redirect Handler"},
		{"news-feed", "News Feed"},
		{"already Titled", "Already Titled"},
		{"", ""},
	} {
		if got := titleCase(tc.in); got != tc.want {
			t.Errorf("titleCase(%q) = %q, want %q", tc.in, got, tc.want)
		}
	}
}

// A folder-only board organises the tree without rendering, so it must be skipped while its
// children are still walked — and those children must point past it to the nearest ancestor that
// does render, or a breadcrumb names a board the reader can never be on.
func TestWalkBoardsSkipsFolderOnlyButKeepsChildren(t *testing.T) {
	leaf := &d2target.Diagram{Name: "leaf"}
	folder := &d2target.Diagram{Name: "folder", IsFolderOnly: true, Layers: []*d2target.Diagram{leaf}}
	root := &d2target.Diagram{Name: "root", Layers: []*d2target.Diagram{folder}}

	boards := walkBoards(root, "Overview")
	if len(boards) != 2 {
		t.Fatalf("walked %d boards, want 2 (root + leaf, folder skipped)", len(boards))
	}
	if boards[0].ID != "root" || boards[0].Title != "Overview" {
		t.Errorf("root board = %+v", boards[0].BoardMeta)
	}
	if boards[1].ID != "root.layers.folder.layers.leaf" {
		t.Errorf("leaf id = %q", boards[1].ID)
	}
	if boards[1].Parent == nil || *boards[1].Parent != "root" {
		t.Errorf("leaf parent = %v, want the nearest RENDERED ancestor (root)", boards[1].Parent)
	}
}

// Two boards can share a slug; the walk disambiguates in order, and the manifest is the truth
// about which slug a board actually got.
func TestWalkBoardsDisambiguatesCollidingSlugs(t *testing.T) {
	root := &d2target.Diagram{
		Name: "root",
		Layers: []*d2target.Diagram{
			{Name: "dup"},
			{Name: "dup"},
		},
	}
	boards := walkBoards(root, "")
	if len(boards) != 3 {
		t.Fatalf("walked %d boards, want 3", len(boards))
	}
	if boards[0].Title != "Overview" {
		t.Errorf("an unnamed root is titled %q, want Overview", boards[0].Title)
	}
	if boards[1].Slug != "dup" || boards[2].Slug != "dup-2" {
		t.Errorf("slugs = %q, %q — want dup, dup-2", boards[1].Slug, boards[2].Slug)
	}
}

// Only links into this diagram's own boards are steps in the walk; an href to the web is not.
func TestLinksOfKeepsOnlyInternalTargets(t *testing.T) {
	board := &d2target.Diagram{
		Shapes: []d2target.Shape{
			{Link: "root.layers.a"},
			{Link: "https://example.com"},
			{Link: ""},
			{Link: "root.layers.a"}, // repeated — one entry
		},
		Connections: []d2target.Connection{{Link: "root"}},
	}
	got := linksOf(board)
	if len(got) != 2 || got[0] != "root.layers.a" || got[1] != "root" {
		t.Errorf("linksOf = %v, want [root.layers.a root]", got)
	}
}

func TestManifestForIsTheShapeTheReaderAccepts(t *testing.T) {
	root := &d2target.Diagram{Name: "root"}
	manifest := manifestFor("a -> b", walkBoards(root, ""))
	if manifest.Generator != generatorVersion {
		t.Errorf("generator = %d", manifest.Generator)
	}
	// `decodeManifest` rejects a manifest whose source hash does not match the fence it is
	// rendering, so this is the field that decides whether a walkthrough appears at all.
	if manifest.Source != fnv1a("a -> b") {
		t.Errorf("source = %q, want %q", manifest.Source, fnv1a("a -> b"))
	}
	// …and it rejects one whose root names no board in the list.
	found := false
	for _, board := range manifest.Boards {
		if board.ID == manifest.Root {
			found = true
		}
	}
	if !found {
		t.Errorf("root %q names no board", manifest.Root)
	}
	if manifest.Warnings == nil {
		t.Error("warnings must marshal as [], never null")
	}
}

// ── THE ENGINE ───────────────────────────────────────────────────────────────

func TestRenderSourceProducesAnSVGCarryingTheSalt(t *testing.T) {
	svg, err := renderSource(context.Background(), "a -> b", "d2-testsalt")
	if err != nil {
		t.Fatalf("render: %v", err)
	}
	text := string(svg)
	if !strings.HasPrefix(strings.TrimSpace(text), "<svg") {
		t.Fatalf("output does not start with <svg: %.80q", text)
	}
	// noXMLTag: this is embedded into HTML, not written to a file.
	if strings.Contains(text, "<?xml") {
		t.Error("output carries an XML declaration")
	}
	// Not asserted here: that the salt appears in the output. Measured against the wasm build of
	// this same engine, v0.7.0 emits it for no source tried — it derives element ids from the
	// diagram's own hash instead. Both engines are handed the same salt, so they agree either way,
	// and `TestGoAndWasmAgree` in the harness is what actually pins the output.
	if !strings.Contains(text, "d2-svg") {
		t.Error("output carries no d2 markup")
	}
}

func TestRenderSourceRejectsAMalformedDiagram(t *testing.T) {
	// A loud failure is the contract: the caller turns it into a fallback, never a blank figure.
	if _, err := renderSource(context.Background(), "a -> ", "d2-x"); err == nil {
		t.Error("a malformed diagram rendered without error")
	}
}
