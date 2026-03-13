# Divergences from R

This document tracks where miniR intentionally diverges from GNU R behavior.

## Suggestions

- [ ] allow trailing , in lists and `c()`
- [ ] fix documented `c.factor` issue

## Planned

- `readRDS()` / `saveRDS()` / `load()` / `save()` currently use a miniR-specific
  text format (`miniRDS1`) instead of GNU R's binary RDS/XDR and `.RData`
  formats. Files round-trip common miniR values and named workspaces within
  miniR, but they are not compatible with GNU R yet.
