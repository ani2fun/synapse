package main

// ── THE BOARD MODEL ──────────────────────────────────────────────────────────
// A ```d2 boards fence is one source that compiles to a TREE of boards. This is the renderer's
// half of `web/src/lib/islands/diagram/boards.ts` — what a board is called, which slug holds it,
// and what the reader's manifest says. The viewer's half (history, index, URL) stays in TypeScript.
//
// The two must agree exactly. A slug that differs by one character names a board the viewer never
// asks for, and `decodeManifest` drops a manifest whose `source` hash disagrees — both silently,
// by falling back to the client renderer. `boards_test.go` pins the shapes that can drift.

import (
	"regexp"
	"strconv"
	"strings"

	"oss.terrastruct.com/d2/d2target"
)

// The manifest version the reader understands. Bump only alongside `GENERATOR_VERSION` in
// boards.ts — an unrecognised version is treated as "not drawn", which is the safe direction.
const generatorVersion = 1

const rootID = "root"

// Only `root=` is read here. Whether a fence is a walkthrough at all, and what its `name=` is, are
// the CALLER's business: the page tier decides which endpoint to POST to, and the name never
// reaches the manifest. Re-deriving either would be a second opinion nobody consults.
var rootMeta = regexp.MustCompile(`(?:^|\s)root=(?:"([^"]*)"|(\S+))`)

// The segments a board id carries for structure rather than meaning.
var boardKinds = []string{"layers", "steps", "scenarios"}

func isBoardKey(part string) bool {
	for _, kind := range boardKinds {
		if part == kind {
			return true
		}
	}
	return false
}

// firstGroup returns the quoted or bare capture, whichever the author wrote.
func firstGroup(m []string) string {
	if m == nil {
		return ""
	}
	if m[1] != "" {
		return m[1]
	}
	return m[2]
}

// rootTitleOf reads `root="System Context"` — the root board's title, which has no key to derive
// one from. Empty when unset; the walk then calls it "Overview".
func rootTitleOf(meta string) string { return firstGroup(rootMeta.FindStringSubmatch(meta)) }

// boardSlug turns a board id into its filename stem: `root.layers.container` → `container`.
//
// The kind segments carry no information a reader needs, so they are dropped and the remaining
// keys joined. Two boards can therefore collide; `walkBoards` disambiguates with a suffix when it
// assigns slugs, so this stays a pure function of one id.
func boardSlug(id string) string {
	parts := make([]string, 0, 4)
	for _, part := range strings.Split(id, ".") {
		if part == "" || part == rootID || isBoardKey(part) {
			continue
		}
		parts = append(parts, part)
	}
	joined := rootID
	if len(parts) > 0 {
		joined = strings.Join(parts, "-")
	}
	var out strings.Builder
	for _, r := range strings.ToLower(joined) {
		if (r >= 'a' && r <= 'z') || (r >= '0' && r <= '9') || r == '_' || r == '-' {
			out.WriteRune(r)
		} else {
			out.WriteRune('-')
		}
	}
	// Collapse the runs the character map just created, then trim: `a  b` and `a--b` are one slug.
	clean := strings.Trim(collapseDashes(out.String()), "-")
	// Everything downstream treats a slug as a path segment, so an id made entirely of punctuation
	// has to become a name rather than an empty string or a traversal.
	if clean == "" {
		return "board"
	}
	return clean
}

func collapseDashes(value string) string {
	var out strings.Builder
	previousDash := false
	for _, r := range value {
		if r == '-' {
			if !previousDash {
				out.WriteRune(r)
			}
			previousDash = true
			continue
		}
		previousDash = false
		out.WriteRune(r)
	}
	return out.String()
}

// saltForBoard is the id suffix one board's SVG carries. Unique per board, so two boards on a page
// cannot collide on `<defs>` ids — a collision loses arrowheads and clips with no error anywhere.
func saltForBoard(sourceHash, id string) string {
	return "d2-" + sourceHash + "-" + boardSlug(id)
}

// titleCase turns `redirect_handler` into `Redirect Handler`. A layer's key is the only title d2
// offers for it.
func titleCase(key string) string {
	fields := strings.FieldsFunc(key, func(r rune) bool {
		return r == '-' || r == '_' || r == ' ' || r == '\t' || r == '\n'
	})
	words := make([]string, 0, len(fields))
	for _, field := range fields {
		runes := []rune(field)
		words = append(words, strings.ToUpper(string(runes[0]))+string(runes[1:]))
	}
	return strings.Join(words, " ")
}

// BoardMeta is one board as the viewer knows it.
type BoardMeta struct {
	ID     string   `json:"id"`
	Slug   string   `json:"slug"`
	Title  string   `json:"title"`
	Parent *string  `json:"parent"`
	Links  []string `json:"links"`
}

// BoardManifest is one fence's board graph, in the shape `decodeManifest` accepts.
type BoardManifest struct {
	Generator int         `json:"generator"`
	Source    string      `json:"source"`
	Root      string      `json:"root"`
	Boards    []BoardMeta `json:"boards"`
	// Always empty here. The dead-`link:` audit was a property of the CLI that walked a whole
	// checkout and could name a file and line; on-demand rendering sees one fence with no
	// position, so the diagnostic belongs to the authoring surfaces (`/d2`), not to this.
	Warnings []BoardWarning `json:"warnings"`
}

// BoardWarning is a `link:` the author wrote that names no board. The reader tolerates any array
// here, so the shape matters only if something starts filling it in.
type BoardWarning struct {
	Value string  `json:"value"`
	Board string  `json:"board"`
	Line  int     `json:"line"`
	Hint  *string `json:"hint"`
}

// walkedBoard carries the compiled node, so rendering one costs a render and no second compile.
type walkedBoard struct {
	BoardMeta
	node *d2target.Diagram
}

// linksOf collects the `link:` targets that point at another board of this diagram. An external
// href is a link to the web, not a step in the walk.
func linksOf(board *d2target.Diagram) []string {
	out := make([]string, 0, 4)
	seen := map[string]bool{}
	add := func(link string) {
		if link == "" || seen[link] {
			return
		}
		if link != rootID && !strings.HasPrefix(link, rootID+".") {
			return
		}
		seen[link] = true
		out = append(out, link)
	}
	for _, shape := range board.Shapes {
		add(shape.Link)
	}
	for _, connection := range board.Connections {
		add(connection.Link)
	}
	return out
}

// boardChild is one board and the kind segment its id carries.
type boardChild struct {
	kind  string
	child *d2target.Diagram
}

// childrenOf returns one node's boards in the order boards.ts walks them: layers, then steps,
// then scenarios. The order decides slug disambiguation suffixes, so it is part of the contract.
func childrenOf(node *d2target.Diagram) []boardChild {
	out := make([]boardChild, 0, 8)
	for _, group := range []struct {
		kind     string
		children []*d2target.Diagram
	}{
		{"layers", node.Layers},
		{"steps", node.Steps},
		{"scenarios", node.Scenarios},
	} {
		for _, child := range group.children {
			if child == nil {
				continue
			}
			out = append(out, boardChild{group.kind, child})
		}
	}
	return out
}

// walkBoards returns the compiled diagram's boards, depth first, root first.
//
// A folder-only board organises the tree without rendering anything, so it is skipped while its
// children are still walked — and those children point past it to the nearest ancestor that does
// render, which is what keeps a breadcrumb meaningful.
func walkBoards(diagram *d2target.Diagram, rootTitle string) []walkedBoard {
	boards := make([]walkedBoard, 0, 8)

	var walk func(node *d2target.Diagram, id, title string, parent *string)
	walk = func(node *d2target.Diagram, id, title string, parent *string) {
		nearest := parent
		if !node.IsFolderOnly {
			boards = append(boards, walkedBoard{
				BoardMeta: BoardMeta{ID: id, Title: title, Parent: parent, Links: linksOf(node)},
				node:      node,
			})
			own := id
			nearest = &own
		}
		for _, entry := range childrenOf(node) {
			walk(entry.child, id+"."+entry.kind+"."+entry.child.Name, titleCase(entry.child.Name), nearest)
		}
	}

	if rootTitle == "" {
		rootTitle = "Overview"
	}
	walk(diagram, rootID, rootTitle, nil)

	taken := map[string]int{}
	for i := range boards {
		base := boardSlug(boards[i].ID)
		taken[base]++
		if nth := taken[base]; nth == 1 {
			boards[i].Slug = base
		} else {
			boards[i].Slug = base + "-" + strconv.Itoa(nth)
		}
	}
	return boards
}

// manifestFor is the board graph the reader receives, minus the compiled nodes.
func manifestFor(source string, boards []walkedBoard) BoardManifest {
	metas := make([]BoardMeta, 0, len(boards))
	for _, board := range boards {
		meta := board.BoardMeta
		if meta.Links == nil {
			meta.Links = []string{}
		}
		metas = append(metas, meta)
	}
	return BoardManifest{
		Generator: generatorVersion,
		Source:    fnv1a(source),
		Root:      rootID,
		Boards:    metas,
		Warnings:  []BoardWarning{},
	}
}
