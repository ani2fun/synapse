package main

import (
	"fmt"
	"unicode/utf16"
)

// fnv1a is `web/src/lib/hash.ts`, in Go.
//
// It hashes UTF-16 CODE UNITS rather than runes, which is the whole reason it is written out
// instead of reached for from the standard library: the TypeScript original walks `charCodeAt`,
// and this catalog's diagrams are full of `·`, `→` and `≈`. Ranging over a Go string yields runes,
// so a `→` would contribute one value here and two there, and every hash over a diagram
// containing one would disagree.
//
// That disagreement is invisible where it hurts. A walkthrough's manifest carries the hash of the
// source it was built from and the reader drops the manifest when it does not match, so a
// rune-based version would fail on exactly the diagrams that have interesting labels — quietly,
// by falling back to the client renderer.
func fnv1a(input string) string {
	hash := uint32(0x811c9dc5)
	for _, unit := range utf16.Encode([]rune(input)) {
		hash ^= uint32(unit)
		hash *= 0x01000193
	}
	return fmt.Sprintf("%08x", hash)
}
