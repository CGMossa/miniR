# textwrap integration plan

> `textwrap` 0.16 -- Word wrapping, indenting, and dedenting strings.
> <https://github.com/mgeisler/textwrap>

## What it does

Text wrapping and formatting with Unicode awareness. Handles word boundaries,
hyphenation, indentation, and terminal width detection.

```rust
use textwrap::{wrap, indent, dedent, fill};

let text = "This is a long string that should be wrapped at 40 columns.";
let wrapped = wrap(text, 40);
// ["This is a long string that should", "be wrapped at 40 columns."]

let filled = fill(text, 40);
// "This is a long string that should\nbe wrapped at 40 columns."

let indented = indent("line1\nline2", "  ");
// "  line1\n  line2"
```

Features:
- Unicode-aware line breaking (via `unicode-linebreak`)
- Terminal width detection (via `terminal_size`)
- Optimal line breaking (Knuth-Plass algorithm via `smawk`)
- Optional hyphenation

## Where it fits in miniR

### `strwrap()` -- wrap character strings

```r
strwrap("This is a long string", width = 40)
# [1] "This is a long string"

strwrap("This is a long string that needs wrapping", width = 20)
# [1] "This is a long"
# [2] "string that needs"
# [3] "wrapping"

strwrap(x, width = 40, indent = 2, exdent = 4)
```

Parameters:
- `width` -- target line width (default: `getOption("width")`)
- `indent` -- indentation of first line
- `exdent` -- indentation of subsequent lines
- `prefix` / `initial` -- line prefixes
- `simplify` -- whether to return a single vector or list

### `formatDL()` -- format definition lists

```r
formatDL(tags = c("foo", "bar"), descs = c("description 1", "description 2"))
```

Uses text wrapping internally for the description column.

### Help text formatting

R's `help()` system formats man pages with text wrapping. When we implement
`?.topic` or `help(topic)`, textwrap handles the display.

### `cat()` / `message()` wrapping

Long messages and warnings can be auto-wrapped to terminal width.

## Implementation

1. Implement `strwrap(x, width, indent, exdent, prefix, initial, simplify)` in builtins
2. Use `textwrap::Options` for full control over wrapping behavior
3. Wire `width` parameter to `getOption("width")` default
4. Implement `formatDL()` using strwrap internally

## Status

Already vendored as a transitive dependency (via miette). Not currently
a direct dependency -- would need to be added to Cargo.toml if used directly.

## Priority

Medium -- `strwrap()` is used by many packages for formatting output,
and `formatDL()` is part of R's help system. Nice to have for display
quality.
