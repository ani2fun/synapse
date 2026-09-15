---
title: "Threshold, Rewritten"
summary: "The smoke problem, rewritten in a private book — the workbench must come with it."
kind: problem
difficulty: easy
topics: [smoke]
---

# Threshold, Rewritten

Given an integer n, print `Over` if n >= 10, else `Under`. The zebra invariant: one comparison decides.

## Example 1

**Input:** n = 12

**Output:** Over

```python run
n = int(input())
print("Over" if n >= 10 else "Under")
```

```testcases
{
  "args": [
    { "id": "n", "label": "n", "type": "int", "placeholder": "12" }
  ],
  "cases": [
    { "args": { "n": "12" }, "expected": "Over" },
    { "args": { "n": "3" }, "expected": "Under" }
  ]
}
```
