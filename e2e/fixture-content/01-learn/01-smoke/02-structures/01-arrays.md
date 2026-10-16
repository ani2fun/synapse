---
title: Arrays
summary: A second lesson, so reading order, the sidebar and prev/next all have somewhere to go.
---

# Arrays

A second lesson gives the book a reading order, which the sidebar renders, the progress chip
counts against, and the resume card points into. Without it the fixture could not exercise
"2 of N read" at all.

An array stores its elements contiguously, so indexing is constant time and inserting in the
middle is not. That is the whole trade, and it is enough prose for the body assertion.

Unlike the satellite guide, which arrives from a repository of its own, this book is the spine.

## Indexing

An array is one block of memory with a fixed element size, so the address of any element is the base address plus the index times that size. Nothing has to be searched or followed: the arithmetic is the lookup, and it costs the same for the first element as for the millionth.

An array is one block of memory with a fixed element size, so the address of any element is the base address plus the index times that size. Nothing has to be searched or followed: the arithmetic is the lookup, and it costs the same for the first element as for the millionth.

An array is one block of memory with a fixed element size, so the address of any element is the base address plus the index times that size. Nothing has to be searched or followed: the arithmetic is the lookup, and it costs the same for the first element as for the millionth.

## Insertion

Inserting in the middle is the price of that layout. Every element after the insertion point moves one slot to the right to make room, so the cost grows with how much of the array sits behind the new element, and an append to a full array copies the whole block elsewhere first.

Inserting in the middle is the price of that layout. Every element after the insertion point moves one slot to the right to make room, so the cost grows with how much of the array sits behind the new element, and an append to a full array copies the whole block elsewhere first.

### Shifting

Inserting in the middle is the price of that layout. Every element after the insertion point moves one slot to the right to make room, so the cost grows with how much of the array sits behind the new element, and an append to a full array copies the whole block elsewhere first.

Inserting in the middle is the price of that layout. Every element after the insertion point moves one slot to the right to make room, so the cost grows with how much of the array sits behind the new element, and an append to a full array copies the whole block elsewhere first.

## Trade-offs

The trade is the whole story of the structure, and it is why other structures exist at all. A linked list gives up the arithmetic to make insertion cheap; a dynamic array keeps the arithmetic and amortises the copying. Choosing between them is choosing which cost to pay.

The trade is the whole story of the structure, and it is why other structures exist at all. A linked list gives up the arithmetic to make insertion cheap; a dynamic array keeps the arithmetic and amortises the copying. Choosing between them is choosing which cost to pay.

The trade is the whole story of the structure, and it is why other structures exist at all. A linked list gives up the arithmetic to make insertion cheap; a dynamic array keeps the arithmetic and amortises the copying. Choosing between them is choosing which cost to pay.

The trade is the whole story of the structure, and it is why other structures exist at all. A linked list gives up the arithmetic to make insertion cheap; a dynamic array keeps the arithmetic and amortises the copying. Choosing between them is choosing which cost to pay.
