# unicode-width integration plan

> `unicode-width` 0.2 — Determine displayed width of Unicode characters.
> Already vendored as a transitive dependency of reedline.

## What it does

Returns the number of terminal columns a character occupies:

- ASCII and most Latin/Cyrillic: width 1
- CJK ideographs, fullwidth forms: width 2
- Zero-width joiners, combining marks: width 0
- Control characters: width 0 (or None)

## Where it fits in miniR

### R functions

| R function | unicode-width API |
| ---------- | ----------------- |
| `nchar(x, type="width")` | `UnicodeWidthStr::width(s)` |
| `format(x, width=10)` | Pad to target display width using char widths |
| `formatC(x, width=10)` | Same |
| `cat()` with `fill=` | Line wrapping by display width |
| `print.data.frame()` | Column alignment by display width |
| `print.matrix()` | Column alignment by display width |

### The problem

Our current `nchar(x, type="width")` likely uses byte or char count. CJK characters display as 2 columns wide. Without proper width calculation, data frame printing with CJK column names or values will be misaligned.

### Example

```rust
use unicode_width::UnicodeWidthStr;

fn r_nchar_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

fn r_format_right(s: &str, target_width: usize) -> String {
    let current = UnicodeWidthStr::width(s);
    if current >= target_width {
        s.to_string()
    } else {
        format!("{}{}", " ".repeat(target_width - current), s)
    }
}
```

## Implementation order

1. Fix `nchar(x, type="width")` to use `UnicodeWidthStr::width()`
2. Use display width in `format()` / `formatC()` padding
3. Use display width in data frame column alignment
4. Use display width in matrix print alignment

## Priority

Medium — correctness for international text. Already vendored, zero cost to integrate.
