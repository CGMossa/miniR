# Divergences from R

This document tracks where miniR intentionally diverges from GNU R behavior.

## Suggestions

- [ ] allow trailing , in lists and `c()`
- [ ] fix documented `c.factor` issue

## Planned

- `readRDS()` / `saveRDS()` currently use a miniR-specific text format (`miniRDS1`)
  instead of GNU R's binary RDS/XDR format. Files round-trip common miniR values
  within miniR, but they are not compatible with GNU R yet.
