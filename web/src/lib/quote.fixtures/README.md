Live captures of the two candidate quote feeds, taken 2026-09-04, byte for byte.

They are the parser's test input (`../quote.test.ts`) so that `parseFeed` is proved against what
the publishers actually send rather than against a hand-written idea of RSS — the two differ in
whitespace, in element order and in whether the body is one line or indented, and both shapes have
to parse. `brainyquote.rss` is the feed the header reads; `azquotes.rss` is kept because it is the
formatted counter-example, and because it documents why it was NOT chosen (no `pubDate` anywhere).

Nothing at runtime reads these.
