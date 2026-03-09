# tabwriter integration plan

> `tabwriter` 1.4 — Elastic tabstops text formatting by BurntSushi.
> <https://github.com/BurntSushi/tabwriter>

## What it does

Aligns columns in text using elastic tabstops. Write tab-separated data, and
tabwriter pads columns to align:

```rust
use tabwriter::TabWriter;
use std::io::Write;

let mut tw = TabWriter::new(vec![]);
write!(&mut tw, "Name\tAge\tCity\n")?;
write!(&mut tw, "Alice\t30\tNew York\n")?;
write!(&mut tw, "Bob\t25\tSan Francisco\n")?;
tw.flush()?;
// Output:
// Name   Age  City
// Alice  30   New York
// Bob    25   San Francisco
```

## Where it fits in newr

### 1. `print.data.frame()` — formatted table output

R's default data frame printing aligns columns:

```r
> data.frame(name=c("Alice","Bob"), age=c(30,25))
   name age
1 Alice  30
2   Bob  25
```

tabwriter handles the column alignment without manual width calculation.

### 2. `format()` — general tabular formatting

R's `format()` on matrices and tables produces aligned output. tabwriter
provides the alignment engine.

### 3. `cat()` with `\t` — tab-separated output

When users use `cat()` with tabs, tabwriter can optionally align the output.

## Relationship to builtins plan

| Phase | Builtins affected | Impact |
|---|---|---|
| Core (display) | `print.data.frame()`, `print.matrix()` | aligned table output |
| Phase 2 (strings) | `format()`, `formatC()` | tabular formatting |

## Recommendation

**Add when implementing data frame printing.** Simple utility that eliminates
manual column-width calculation. However, R's column alignment has specific rules
(right-align numbers, left-align strings) that may need custom logic on top.

**Effort:** 30 minutes to integrate into print.data.frame.
