# Maintainer Notes

## Required tools

- **Rust** (stable + nightly) — nightly needed for `just crate-docs`
- **just** — task runner (<https://just.systems>)
- **jq** — JSON processor, used by `just crate-docs`
- **cargo-vendor** — ships with Rust, used by `just vendor`

## Useful recipes

- `just vendor` — re-vendor crate dependencies when `Cargo.lock` changes
- `just crate-docs <crate>` — dump public API of a vendored crate as rustdoc JSON summary
- `just update-cran-test-packages` — clone top CRAN packages for compatibility testing
