---
title: Getting started
summary: The first lesson of the e2e fixture book, with enough prose to prove the markdown pipeline ran.
---

# Getting started

This lesson exists so the end-to-end suite has something deterministic to render. Real content
lives in `synapse-content`; pinning the smoke suite to it would mean an unrelated edit there
could turn this repository's CI red, which is the wrong coupling for a smoke test.

It carries a little of everything the reader has to survive: a paragraph long enough that the
body is unmistakably populated rather than merely mounted, a list, a table (tables are what
overflowed the phone in step 46), and a fenced code block.

- A list item, so list styles are exercised.
- A second one.
- A third, for good measure.

| Column | Another | A third |
|---|---|---|
| a value | another value | a third value |
| more | and more | and more again |

```python
def greet(name: str) -> str:
    return f"hello, {name}"
```

That is enough prose to put this comfortably past the length assertion without being a wall of
filler. The suite checks structure and behaviour, never wording.

// Diagram: A single still, captioned by the line above it

![A single still, captioned by the line above it](/media/smoke/frames/step-01.png)

// Interactive Diagram (3 frames): Three stills the reader steps through

![Three stills the reader steps through — frame 1 of 3](/media/smoke/frames/step-01.png)

![Three stills the reader steps through — frame 2 of 3](/media/smoke/frames/step-02.png)

![Three stills the reader steps through — frame 3 of 3](/media/smoke/frames/step-03.png)

// Diagram: A marker with no image under it, which must stay readable prose
