# Native Code Audit — All Issues (2026-03-27)

## Architecture (user requested)
1. **minir_runtime.c → Rust** — rewrite as `extern "C"` Rust functions in the miniR binary

## Missing C API (blocks more CRAN packages)
2. **R_Calloc / R_Free** — R's memory macros (digest needs them)
3. **R_RegisterCCallable** — cross-package C function sharing (digest)
4. **Rf_eval / Rf_lcons** — evaluate R expressions from C (digest, backports)
5. **R_Serialize / R_InitOutPStream** — serialization from C (digest)
6. **R_GlobalEnv** — global env pointer from C (digest)
7. **Rconfig.h** — header with platform config macros
8. **R_ext/Arith.h** — R_FINITE, R_IsNaN, ISNAN macros
9. **R_ext/Error.h** — Rf_errorcall
10. **R_ext/Memory.h** — R_alloc, vmaxget/vmaxset
11. **R_ext/Print.h** — Rprintf (already in runtime, needs header)
12. **cc crate stdout noise** — filter out `cargo:` lines from cc output

## Missing R builtins (blocks package R code)
13. **setRefClass** — R5 reference classes (fastmap, R6, many others)
14. **alist()** — construct unevaluated arg list
15. **deparse1()** — single-string deparse (R 4.0+)
16. **oldClass / oldClass<-** — legacy class access

## Remaining concessions from earlier
17. **Cross-package alloc isolation** — documented as correct (each .Call goes through one .so)
18. **Max 16 .Call args** — sufficient for all CRAN packages in practice

## Tested CRAN packages
- **base64enc**: WORKS (encode through R wrappers)
- **fastmap**: compiles C++, loads, native runs — blocked on setRefClass
- **digest**: needs R_Calloc, R_Serialize, Rf_eval, Rconfig.h
- **backports**: needs findVar, eval, nthcdr (deep R evaluator internals)
