# Step 38 — Toolbar icons + the Visualise modal, finished

*(oracle: the post-33 toolbar polish + `VisualiseModal`'s final layout — the parity list's
items 3–5, closing the user's screenshot set.)*

## The toolbar (screenshot 3)

Every verb now wears its oracle icon: the language pill is **▶ name** — and with several
variants it becomes the **chevron dropdown** (scrim + menu, each option with its own play
glyph) replacing the step-30 tabs; Edit carries the lock (label swaps to plain "Editing"
when unlocked); the **↺ Reset** icon-button appears while editing and restores the
starter; Visualise carries the eye; Submit the rocket; Run the play triangle with its —
now visible — label. The floating **copy-code** button hovers top-right over every editor,
reads the LIVE buffer, and swaps to a check for 1.4 s. (The invisible-Run-label root cause
— primary-at-8%-alpha text on a primary background — was fixed in step 36.)

## The modal (screenshots 4–5)

The canvas card gained its controls row: **− % +** zoom (0.5–4.0 in 0.25 steps, the %
resets, `F` too) applied as CSS `zoom` on the host's scale layer — layout-aware, so the
scroll area grows and centering holds — and the **◧ Diff** toggle (`D`): when on, the
step buttons and arrow keys hop only between steps that CHANGED the structure (the wire's
`unchanged` flag), while the scrubber, label, and autoplay still cover every step. Under
the card, the **numbered timeline** reaches any step regardless of diff mode, greying the
structurally-unchanged ones. Below both columns, the **Program output** collapsible and
the **editable stdin** whose "↻ Re-trace with this input" (and `r`) runs a FRESH trace
keyed on the new stdin — a new session, distinct from the top bar's plain Re-trace. The
bar gained the **(i) guide** — the four-section "How Visualise works" card, oracle copy
verbatim. The canvas column now holds 40–64vh with the widget centered (the
screenshot-5 fix landed in step 36; the height/centering overrides land here). The
keyboard ignores typing surfaces and covers Space · ←/→ · `r` · `f` · `d`.

## Verified live

The queue trace: zoom + → 125% with `zoom: 1.25` on the scale layer; Diff on hopped 1 → 2
(the first structural change); the greyed chip 9 still jumped straight there (chips bypass
diff); next-from-9 hopped to 10; the guide opened with its four sections; the stdin panel
present; Esc closed. The toolbar: ▶ Python pill → dropdown listing Python · Java → picking
Java swapped the pill and closed the menu; lock/eye/play icons and the floating copy
button all live. Suite: 361 Rust + 44 vitest; bundle 606/700 KiB gz.

The five-screenshot parity list is closed.
