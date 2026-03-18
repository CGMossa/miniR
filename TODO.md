# TODO — Remaining Work

## Everything done

- [x] Package runtime (library, require, loadNamespace, detach, .onLoad, .onAttach, system.file, packageVersion, collation order, S3method registration)
- [x] Serialization (readRDS, saveRDS, load, save, save.image — XDR binary, ASCII format, gzip, closures/environments)
- [x] 650+ builtins verified working (quantile, rep each, sample prob, sort na.last, diff differences, ordered, makeActiveBinding, on.exit, all.equal, identical, invisible, blake3, ChaCha20, progress bars, 48 distributions with lower.tail/log.p/log)
- [x] Architecture (tracing, nalgebra, session-scoped output, build profiles)
- [x] Parser (7014/7014 CRAN files, :=, ~~, backtick operators, raw string dashes)
- [x] S4 OOP (class registry, method dispatch, slot validation)
- [x] Rd documentation parser (1717 lines, wired into help())
- [x] REPL (syntax highlighting, completion, continuation detection, terminal width)
- [x] Bug fixes (ifelse, replace, round, log base, cumsum NA, var/sd/median na.rm, is.* predicates, sign(0), match.arg, rm NSE, format digits, aggregate formula, substitute NSE, print.matrix, print.factor, summary quartiles, str details)

## Open

### Near-term

- [ ] Synthesize Rd help pages from builtin rustdoc at init (see `plans/rustdoc-to-rd.md`)
- [ ] `pnorm()` precision — erfc approximation gives ~1.5e-7 accuracy, could use a better algorithm
- [ ] `Sys.time()` improvements — subsecond precision verification

### Graphics

- [ ] egui-based interactive plotting (see `plans/egui-graphics.md`)
- [ ] SVG/PNG file device output

### Deferred

- [ ] Arrow backend for vector storage
- [ ] WASM target support (test with `--no-default-features -F minimal`)
- [ ] Full S4 inheritance chain resolution (basic registry works)
- [ ] Polars-backed data frames
