package main

// ── THE ENGINE ───────────────────────────────────────────────────────────────
// d2 source → SVG, with the render contract of `web/src/lib/islands/diagram/d2.ts` reproduced
// exactly: ELK layout, themeID 0, pad 20, no XML tag, and the caller's salt.
//
// Those constants are not preferences. The `/d2` editor previews with the wasm build of the same
// engine, so a diagram an author tidies there and a diagram a reader sees must come out the same;
// a different pad or theme makes the editor lie about what will ship.
//
// Always the light neutral theme, independent of the reader's page theme: authored diagrams
// colour their nodes with a fixed light palette and never set a label colour, so a dark theme
// paints light text on light fills.

import (
	"context"
	"fmt"
	"log/slog"
	"os"
	"sync"

	"oss.terrastruct.com/d2/d2graph"
	"oss.terrastruct.com/d2/d2layouts/d2elklayout"
	"oss.terrastruct.com/d2/d2lib"
	"oss.terrastruct.com/d2/d2renderers/d2svg"
	"oss.terrastruct.com/d2/d2target"
	d2log "oss.terrastruct.com/d2/lib/log"
	"oss.terrastruct.com/d2/lib/textmeasure"
)

// d2 logs through a slog.Logger it expects to find on the context, and prints a warning WITH A
// GOROUTINE STACK on every single compile when it does not find one. Error level: the engine's
// debug chatter is per-diagram and says nothing an operator of this service can act on, while a
// real failure already comes back as an error to the caller.
var engineLog = slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: slog.LevelError}))

const (
	renderThemeID int64 = 0
	renderPad     int64 = 20
)

// engineMu serialises every compile and render.
//
// d2's ELK layout is a wrapper around the JavaScript port of ELK and carries a JS runtime with it;
// nothing documents it as safe to drive from several goroutines, and the wasm build of this same
// engine is explicitly single-flight. Renders cost tens of milliseconds and every result is cached
// forever, so serialising costs throughput nobody is asking for and removes a class of bug that
// would show up as a corrupted figure rather than an error.
var engineMu sync.Mutex

// One ruler, built once. Text measurement is the slowest part of a cold render and the ruler is
// immutable once built; it is only ever touched under engineMu.
var (
	rulerOnce sync.Once
	ruler     *textmeasure.Ruler
	rulerErr  error
)

func renderOpts(salt string) *d2svg.RenderOpts {
	theme, padding, noXML := renderThemeID, renderPad, true
	return &d2svg.RenderOpts{
		ThemeID:  &theme,
		Pad:      &padding,
		NoXMLTag: &noXML, // embedding into HTML, not writing a file
		Salt:     &salt,
	}
}

func compileOpts() (*d2lib.CompileOptions, error) {
	rulerOnce.Do(func() { ruler, rulerErr = textmeasure.NewRuler() })
	if rulerErr != nil {
		return nil, fmt.Errorf("text ruler: %w", rulerErr)
	}
	engine := "elk"
	return &d2lib.CompileOptions{
		Ruler:  ruler,
		Layout: &engine,
		LayoutResolver: func(string) (d2graph.LayoutGraph, error) {
			// One engine, named rather than resolved: `elk` is the only layout this catalog is
			// drawn with, and silently falling back to dagre would relayout every diagram.
			return d2elklayout.DefaultLayout, nil
		},
	}, nil
}

// compileDiagram compiles one source into its board tree, laid out. A ```d2 boards fence returns a
// diagram carrying layers/steps/scenarios; an ordinary fence returns one board with no children.
func compileDiagram(ctx context.Context, source string) (*d2target.Diagram, error) {
	opts, err := compileOpts()
	if err != nil {
		return nil, err
	}
	engineMu.Lock()
	defer engineMu.Unlock()
	diagram, _, err := d2lib.Compile(d2log.With(ctx, engineLog), source, opts, renderOpts(""))
	if err != nil {
		return nil, err
	}
	return diagram, nil
}

// renderDiagram turns an already-compiled board into an SVG. Split from the compile because a
// walkthrough compiles once and renders each board the reader actually opens.
func renderDiagram(diagram *d2target.Diagram, salt string) ([]byte, error) {
	engineMu.Lock()
	defer engineMu.Unlock()
	return d2svg.Render(diagram, renderOpts(salt))
}

// renderSource is the whole job for an ordinary fence.
func renderSource(ctx context.Context, source, salt string) ([]byte, error) {
	diagram, err := compileDiagram(ctx, source)
	if err != nil {
		return nil, err
	}
	return renderDiagram(diagram, salt)
}
