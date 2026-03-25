# blake3 integration plan

> `blake3` — Fast cryptographic hash function.
> <https://github.com/BLAKE3-team/BLAKE3>

## What it does

BLAKE3 is a cryptographic hash function that is:
- Much faster than SHA-256 (3-5x on most hardware)
- Parallelizable (can use SIMD and multithreading)
- Produces 256-bit hashes by default (extendable output)
- Suitable for both cryptographic and non-cryptographic use

## Where it fits in miniR

### `digest(x, algo = "blake3")`

Add BLAKE3 as an algorithm option for the existing `digest()` builtin.
Currently we support "sha256" and "sha512" via the sha2 crate.
BLAKE3 would be the fastest option for large data hashing.

### File checksums

`tools::md5sum()` / `tools::sha256sum()` equivalents — BLAKE3 would
be the recommended fast alternative.

### Data fingerprinting

When comparing data frames or large vectors for equality, BLAKE3
hashing is faster than element-wise comparison for large inputs.

## Implementation

1. Add `blake3 = { version = "1", optional = true }` with feature `blake3`
2. In digest.rs, add a "blake3" arm to the algo match
3. Add `blake3sum(file)` utility function
4. Feature-gated, NOT in default (it's a large crate with optional SIMD)

## Priority: Low — sha2 covers most needs, blake3 is a performance optimization.
