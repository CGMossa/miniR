# aho-corasick optimization plan

> Already vendored as transitive dep of regex.

## What it does

Multi-pattern string matching using the Aho-Corasick algorithm. Searches
for multiple patterns simultaneously in O(n + m) time where n is text
length and m is total pattern length.

## Where it fits in miniR

### `grep(pattern, x)` with multiple patterns

R's `grep` only takes one pattern. But `grepl` with `sapply` over multiple
patterns is common. An `mgrep(patterns, x)` extension could use aho-corasick
to search for all patterns in a single pass.

### `chartr(old, new, x)` optimization

Character translation could use aho-corasick for multi-byte character
sequences instead of char-by-char replacement.

### `strsplit(x, split)` with multiple delimiters

Currently R needs regex for multiple delimiters. Direct multi-pattern
split would be faster for fixed patterns.

## Priority: Low — nice optimization but not blocking any packages.
