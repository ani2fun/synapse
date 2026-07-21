---
title: "Threshold"
summary: "Print Over or Under depending on whether n crosses 10."
kind: problem
difficulty: easy
topics: [smoke]
---

# Threshold

Given an integer n, print `Over` if n >= 10, else `Under`.

## Example 1

**Input:** n = 12

**Output:** Over

## Example 2

**Input:** n = 3

**Output:** Under

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
