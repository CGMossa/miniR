# FromArgs: use going forward, opportunistic migration

## Decision

**Don't** mass-migrate existing builtins. The fn-macro pattern works, and 776
builtins of churn for a pure refactor isn't worth the regression risk.

**Do** use FromArgs for new builtins with 3+ named args, and migrate existing
builtins opportunistically when rewriting them for other reasons.

## When to use which pattern

| Pattern | When to use |
|---|---|
| `#[builtin]` | Simple eager builtins with 1-2 positional args, no named args |
| `#[interpreter_builtin]` | Same but needs ctx (I/O, eval, RNG) |
| `#[derive(FromArgs)]` | 3+ args, named args with defaults, complex signatures |
| `#[pre_eval_builtin]` | Must receive unevaluated `&[Arg]` |
| `stub_builtin!` / `noop_builtin!` | Unimplemented stubs |

The fn macros are more concise for simple builtins (3 lines vs 8). FromArgs
wins when argument decoding is complex enough that the struct makes it clearer
than manual `args[0]` / `named.iter().find()` extraction.

## What FromArgs needs to be usable

The derive exists but has zero users. Before it's useful for real builtins:

### 1. CoerceArg for Option<T>

Optional arguments where `None` = not provided:

```rust
impl<T: CoerceArg> CoerceArg for Option<T>
```

Many builtins check `args.get(1)` and branch. `Option<T>` handles this.

### 2. CoerceArg for Vector and Environment

```rust
impl CoerceArg for Vector       // the whole Vector enum
impl CoerceArg for Environment  // used by ~30 builtins
```

### 3. Rename support

R uses dots in parameter names (`na.rm`). Rust uses underscores. Add
`#[name = "na.rm"]` field attribute:

```rust
#[derive(FromArgs)]
#[builtin(name = "sum")]
struct SumArgs {
    x: RValue,
    #[name = "na.rm"]
    #[default(false)]
    na_rm: bool,
}
```

### 4. Dots (variadic) support — deferred

~50 builtins use `...`. Adding a `Dots` type to capture remaining positional
args is non-trivial macro work. Defer until a concrete need arises. Variadic
builtins should stay on fn macros for now.

### 5. Nullable<T> — deferred

Distinguishing NULL from missing matters for ~10 builtins. Defer until needed.

## Implementation order

1. Add `CoerceArg` for `Option<T>` — unlocks optional args
2. Add `CoerceArg` for `Vector` and `Environment`
3. Add `#[name = "..."]` field attribute to the derive macro
4. Write one real builtin using FromArgs as a proof of concept (pick something
   with 3-4 named args, like `formatC` or `readLines`)
5. Update CLAUDE.md with guidance on when to use which pattern
6. Add tests for the new CoerceArg impls

## Builtins with 3+ params (172 total — candidates for FromArgs)

These are the builtins where FromArgs would actually reduce complexity. Grouped
by file and sorted by param count. `[E]` = eager, `[I]` = interpreter-aware.

### grid.rs (11 builtins, 3-12 params) — highest value

Grid graphics builtins have the most complex signatures in the codebase.

| Params | Name | Notes |
|---|---|---|
| 12 | `viewport` | x, y, width, height, just, xscale, yscale, angle, clip, gp, layout, name |
| 10 | `gpar` | col, fill, lwd, lty, fontsize, font, fontfamily, lineheight, cex, alpha |
| 10 | `grid.rect` | x, y, width, height, just, default.units, gp, vp, name, draw |
| 10 | `grid.text` | label, x, y, just, rot, default.units, gp, vp, name, draw |
| 9 | `grid.points` | x, y, pch, size, default.units, gp, vp, name, draw |
| 9 | `grid.segments` | x0, y0, x1, y1, default.units, gp, vp, name, draw |
| 8 | `grid.circle` | x, y, r, default.units, gp, vp, name, draw |
| 7 | `grid.lines` | x, y, default.units, gp, vp, name, draw |
| 7 | `grid.polygon` | x, y, default.units, gp, vp, name, draw |
| 7 | `grid.xaxis` | at, label, main, gp, vp, name, draw |
| 5 | `grid.layout` | nrow, ncol, widths, heights, respect |

### graphics.rs (8 builtins, 3-12 params)

| Params | Name |
|---|---|
| 12 | `plot` |
| 6 | `abline` |
| 5 | `hist` |
| 5 | `points` |
| 4 | `barplot` |
| 4 | `lines` |
| 4 | `title` |
| 3 | `boxplot`, `png`, `svg` |

### stats.rs (30 builtins, 3-5 params)

Distribution functions — highly uniform signatures. All follow `d/p/q<dist>(x, param1, param2, ...)`.

| Pattern | Count | Examples |
|---|---|---|
| `d<dist>(x, p1, p2)` | 9 | dnorm, dbeta, dbinom, dcauchy, df, dgamma, dlnorm, dunif, dweibull |
| `p<dist>(q, p1, p2)` | 9 | pnorm, pbeta, pbinom, pcauchy, pf, pgamma, plnorm, punif, pweibull |
| `q<dist>(p, p1, p2)` | 9 | qnorm, qbeta, qbinom, qcauchy, qf, qgamma, qlnorm, qunif, qweibull |
| misc | 3 | cor, scale, weighted.mean |

These are so uniform a batch macro might be better than individual FromArgs structs.

### strings.rs (14 builtins, 3-6 params)

| Params | Name |
|---|---|
| 6 | `strwrap` |
| 5 | `agrep`, `encodeString`, `formatC`, `grep`, `gsub`, `sub` |
| 4 | `agrepl`, `gregexpr`, `grepl`, `iconv`, `regexec`, `regexpr`, `strsplit` |
| 3 | `chartr`, `format.pval`, `ngettext`, `prettyNum`, `substr`, `trimws` |

### random.rs (17 builtins, 3-4 params)

| Params | Name |
|---|---|
| 4 | `rfrechet`, `rhyper`, `rpert`, `rskewnorm`, `rtriangular`, `sample` |
| 3 | `rbeta`, `rbinom`, `rcauchy`, `rf`, `rgamma`, `rgumbel`, `rinvgauss`, `rlnorm`, `rpareto`, `runif`, `rweibull` |

### interp.rs (13 builtins, 3-5 params)

| Params | Name |
|---|---|
| 5 | `format`, `rapply` |
| 4 | `Reduce`, `aggregate`, `apply`, `mapply`, `vapply` |
| 3 | `Find`, `Position`, `Vectorize`, `assign`, `by`, `do.call`, `lapply`, `makeActiveBinding`, `outer`, `packageDescription`, `reg.finalizer`, `split`, `tapply` |

### io.rs (5 builtins, 3-5 params)

| Params | Name |
|---|---|
| 5 | `write.table` |
| 4 | `saveRDS` |
| 3 | `read.csv`, `read.table`, `scan`, `write.csv` |

### collections.rs (4 builtins, 3 params)

`hashmap_get`, `hashmap_set`, `btreemap_get`, `btreemap_set`

### system.rs (3 builtins, 3-5 params)

`list.files` (5), `system2` (4), `dir.create` (3), `list.dirs` (3)

### builtins.rs main (7 builtins, 3-5 params)

`all.equal` (5), `cut` (5), `cat` (4), `Sys.getenv` (3), `ifelse` (3), `match` (3), `match.arg` (3), `new.env` (3), `replace` (3)

### Others

- `graphics/color.rs`: `rainbow` (6), `gray.colors` (5), `rgb` (5), `hcl` (4), `hsv` (4)
- `dataframes.rs`: `merge` (8)
- `s4.rs`: `setClass` (7), `setMethod` (3), `slot<-` (3)
- `conditions.rs`: `simpleCondition` (3)
- `datetime.rs`: `as.Date` (3), `difftime` (3), `strptime` (3)
- `native_code.rs`: `dyn.load` (4), `library.dynam` (3)
- `connections.rs`: `make.socket` (3), `writeLines` (3)
- `progress.rs`: `txtProgressBar` (3)
- `net.rs`: `download.file` (4)
- `stubs.rs`: `.Defunct` (3), `.Deprecated` (3), `registerS3method` (4)
- `math.rs`: `quantile` (4), `rep` (4), `seq` (4), `sweep` (4), `append` (3), `complex` (3), `diff` (3), `kronecker` (3), `sort` (3)

## Best first migration candidates

If you want to prove out FromArgs on real builtins, these are the best picks:

1. **`formatC`** (5 params, eager, strings.rs) — pure function, no ctx, clear params
2. **`cut`** (5 params, eager, builtins.rs) — already uses CallArgs
3. **`rgb`** (5 params, eager, color.rs) — simple scalar args with defaults
4. **`sample`** (4 params, interpreter, random.rs) — needs ctx for RNG, well-defined params
5. **`dir.create`** (3 params, interpreter, system.rs) — simple, uses CallArgs

Avoid starting with grid/graphics (complex RValue types) or interp.rs (environment
interactions).

## Opportunistic migration targets

When touching these builtins for other reasons, consider migrating to FromArgs:

- **CallArgs users** (113 call sites) — CallArgs is already halfway to FromArgs.
  When fixing a bug in one of these builtins, migrating is low-effort.
- **Builtins with incorrect/missing formals** — if `@param` docs are wrong,
  switching to FromArgs fixes formals for free.
- **New builtins** — always use FromArgs if they have 3+ named args.

## What NOT to do

- Don't mass-migrate working builtins as a standalone project
- Don't delete the fn-macro entry points — they're the right tool for simple cases
- Don't delete CallArgs yet — it serves 113 builtins, migrate them gradually
- Don't add Dots/Nullable infrastructure until there's a concrete user
