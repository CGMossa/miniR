# Graphics Device Gaps

Raster devices (`png()`, `jpeg()`, `bmp()`) and `dev.list()` are implemented
but have these known gaps vs GNU R:

## JPEG encoder

Uses the `image` crate's built-in JPEG encoder, not libjpeg. Quality/output
won't match GNU R's jpeg device exactly but produces valid JPEG files.

## Missing `res`/`units` parameters

GNU R's `png()`/`jpeg()`/`bmp()` accept `res` (DPI) and `units`
("px"/"in"/"cm"). We hardcode 96 DPI and pixels only.

## dev.list() names don't print

The names attribute is set correctly on the returned integer vector, but
the named-integer print formatting doesn't display them. Values are correct
(verified via `cat(dev.list())`).

## Single-device model

Only one device can be open at a time. No device stack, so `dev.set()`,
`dev.next()`, `dev.prev()` aren't meaningful. `dev.list()` returns at most
one entry.

## postscript() not implemented

Low priority — PostScript output is rarely used in modern workflows.

## Build time

`raster-device` is in `default` features, adding resvg + tiny-skia + usvg
to every default build (~3s extra). Could be moved to `full`-only if needed.
