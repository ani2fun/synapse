## Intuition

One comparison against the threshold decides everything — the private editorial says so.

## Approach

1. Read the integer.
2. Compare it against 10.
3. Print the matching word.

## Solution

```python solution time=O(1) space=O(1)
n = int(input())
print("Over" if n >= 10 else "Under")
```
