package main

import "testing"

// Every vector here was produced by the TypeScript `fnv1a` (via dev-tools/render-d2.mjs), not by
// this implementation — a test that records what the Go already does would pin the bug rather than
// the contract.
//
// The last three carry the whole point. `·`, `→` and `🎯` are the characters a rune-based hash gets
// wrong, and this catalog's diagrams are full of the first two: a middle dot is one UTF-16 unit
// and three UTF-8 bytes, and an emoji is two UTF-16 units and one rune. Get this wrong and the
// only symptom is walkthroughs quietly falling back on the diagrams with the nicest labels.
func TestFnv1aMatchesTypeScript(t *testing.T) {
	for _, tc := range []struct{ in, want string }{
		{"", "811c9dc5"},
		{"a -> b", "294b0b8d"},
		{"alpha -> gamma", "7c12a413"},
		{`client -> server: "302 · Location"`, "90f2f905"},
		{`db: "short_code (PK) → long_url"`, "930f6789"},
		{`x: "emoji 🎯 here"`, "6d8dd0f6"},
	} {
		if got := fnv1a(tc.in); got != tc.want {
			t.Errorf("fnv1a(%q) = %s, want %s", tc.in, got, tc.want)
		}
	}
}

func TestFnv1aIsAlwaysEightHexDigits(t *testing.T) {
	// The reader builds filenames and element ids out of this, so a short hash would be a
	// different key for the same diagram rather than a formatting nit.
	for _, in := range []string{"", "a", "a -> b", "x: \"🎯\""} {
		if got := fnv1a(in); len(got) != 8 {
			t.Errorf("fnv1a(%q) = %q, want 8 characters", in, got)
		}
	}
}
