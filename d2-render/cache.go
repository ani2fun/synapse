package main

// ── THE CACHE ────────────────────────────────────────────────────────────────
// A figure is a pure function of its source and the render contract, so an entry is valid exactly
// as long as the diagram is unchanged — an edit is a different key, and nothing ever needs
// invalidating. That is what makes this a plain directory of files and a cold start merely slow.
//
// It replaces `_media/d2/<hash>.svg` committed to a content repository. Same addressing, same
// immutability; it simply stops being something an author has to remember to generate and commit.

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
)

// cacheVersion salts every key with the render contract's generation. Change a pad, a theme or the
// layout engine and every existing entry is wrong — bumping this retires them all at once, rather
// than serving a mix of two contracts until someone clears a volume by hand.
const cacheVersion = "v1"

type diskCache struct {
	dir   string
	locks sync.Map // key → *sync.Mutex
}

func newDiskCache(dir string) (*diskCache, error) {
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return nil, fmt.Errorf("cache dir %s: %w", dir, err)
	}
	return &diskCache{dir: dir}, nil
}

// safeKey guards the one way a key reaches the filesystem. Hashes and slugs are generated, not
// user input, but they arrive over HTTP — so this is the boundary that has to hold rather than
// the callers that happen to be well behaved today.
func safeKey(key string) bool {
	if key == "" || len(key) > 128 {
		return false
	}
	for _, r := range key {
		ok := (r >= 'a' && r <= 'z') || (r >= 'A' && r <= 'Z') || (r >= '0' && r <= '9') ||
			r == '-' || r == '_' || r == '.'
		if !ok {
			return false
		}
	}
	// `.` and `..` pass the character test and are the whole reason for it.
	return !strings.HasPrefix(key, ".")
}

func (c *diskCache) path(key string) string { return filepath.Join(c.dir, cacheVersion+"-"+key) }

func (c *diskCache) get(key string) ([]byte, bool) {
	if !safeKey(key) {
		return nil, false
	}
	data, err := os.ReadFile(c.path(key))
	if err != nil || len(data) == 0 {
		return nil, false
	}
	return data, true
}

// put writes through a temp file in the same directory, so a reader never sees a half-written
// figure: a crash mid-render leaves rubbish nobody reads rather than a truncated SVG under the
// name of a good one.
func (c *diskCache) put(key string, data []byte) error {
	if !safeKey(key) {
		return errors.New("unsafe cache key")
	}
	tmp, err := os.CreateTemp(c.dir, ".tmp-*")
	if err != nil {
		return err
	}
	name := tmp.Name()
	defer os.Remove(name) // a no-op once the rename below has succeeded
	if _, err := tmp.Write(data); err != nil {
		tmp.Close()
		return err
	}
	if err := tmp.Close(); err != nil {
		return err
	}
	return os.Rename(name, c.path(key))
}

// do returns the cached entry, or produces and stores it.
//
// One producer per key: a lesson holding the same undrawn diagram twice, or two readers arriving
// together, would otherwise each pay a full render for one result. The engine serialises anyway,
// so without this the second caller waits for the first render and then does its own.
func (c *diskCache) do(key string, produce func() ([]byte, error)) ([]byte, error) {
	if data, ok := c.get(key); ok {
		return data, nil
	}
	gate, _ := c.locks.LoadOrStore(key, &sync.Mutex{})
	mu := gate.(*sync.Mutex)
	mu.Lock()
	defer mu.Unlock()
	// The winner filled it while this caller waited for the lock.
	if data, ok := c.get(key); ok {
		return data, nil
	}
	data, err := produce()
	if err != nil {
		return nil, err
	}
	if err := c.put(key, data); err != nil {
		// A cache that cannot write still serves correct figures, just slowly. Losing the page
		// over a full disk would be the worse trade.
		logf("cache: %s not stored — %v", key, err)
	}
	return data, nil
}
