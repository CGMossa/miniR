# CRAN Package Sourcing Test — 2026-03-19

Tested how many R source files from real CRAN packages can be
successfully sourced (parsed + evaluated for top-level definitions).

## Results

| Package | Files OK | Total | Rate |
|---|---|---|---|
| stringr | 35 | 35 | 100% |
| jsonlite | 63 | 64 | 98% |
| glue | 10 | 11 | 91% |
| base R (apply, sort, paste) | 3 | 3 | 100% |

## Issues Found

- **rlang**: Stack overflow — deeply nested expressions exceed stack
- **testthat**: "non-numeric argument to binary operator" — likely needs
  operator methods or NSE features
- **jsonlite 1 failure**: Unknown — needs investigation

## Notes

Most failures are from missing rlang/tidyverse infrastructure (tidy
evaluation, data masking, quosures) rather than core R features. The
base R compatibility is solid.
