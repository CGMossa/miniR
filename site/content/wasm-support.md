+++
title = "WASM Support"
weight = 6
description = "How miniR's feature layout supports wasm-friendly builds, what the current blocker is, and which subsystems are intentionally left out of a WASM target"
+++

miniR is designed with a WASM-oriented build shape in mind, but the current repository does not yet have full `wasm32` support end to end.

The important distinction is:

- the feature graph is intentionally structured so a small, embedded build is possible
- one key registration dependency still blocks a working `wasm32-unknown-unknown` build today

## What The WASM Story Is Trying To Achieve

The target is not "the full desktop interpreter in a browser tab."

The realistic WASM goal is a smaller build shape for:

- parser and evaluator experiments
- embedded docs or teaching tools
- sandboxed execution environments
- host applications that do not want native package loading, terminal UI, or GUI devices

That is why the project has `minimal`, `fast`, `default`, and `full` profiles in the first place.

## The Right Starting Point

For WASM-oriented work, the intended starting profile is:

```bash
cargo build --target wasm32-unknown-unknown --no-default-features -F minimal
```

`minimal` strips away optional subsystems such as:

- REPL and terminal UI
- native package compilation and dynamic loading
- TLS-backed HTTPS support
- GUI plotting
- heavier linear algebra dependencies

This keeps the dependency graph closer to what a sandboxed target can actually support.

## The Current Blocker

Today, the main blocker is builtin auto-registration.

miniR registers builtins through `linkme` distributed slices. That works on native targets because the linker can assemble those slices from target-specific linker sections. `wasm32-unknown-unknown` does not provide the same mechanism.

In practical terms, this means:

- the project is shaped for a WASM target
- the builtin registry path is not yet WASM-compatible
- a real WASM build needs either a `linkme` fork or a different registration strategy

See `plans/linkme-wasm.md` and the `WASM Target` section of `TODO.md` for the current implementation direction.

The separate `Builtin Registry And linkme` page explains why builtin auto-registration is the sharp edge here instead of, for example, parsing or ordinary evaluation.

## What Would Still Be Out Of Scope

Even after builtin registration is solved, a WASM target would still intentionally exclude some subsystems:

- `.Call`, `.External`, `dyn.load`, and package native compilation
- host filesystem assumptions
- process-global OS integration points
- desktop plotting windows and related GUI support

That is expected. A WASM build is a different product shape, not "native miniR with a different compiler target."

## Why Feature Flags Matter Here

The reason miniR spends effort on optional features is not only compile speed. It is also architectural pressure:

- host-specific subsystems must stay separable
- the interpreter core must remain usable without them
- embedded targets should not be forced to carry native-only infrastructure

If a feature cannot be cleanly turned off, it becomes much harder to imagine a credible WASM build later.

## Native Support And WASM

The `native` feature gates the compiled-extension pipeline. Without it, calls such as `.Call()` and `dyn.load()` should fail explicitly rather than pretending the capability exists.

That is the right model for WASM too. A sandboxed target should be honest about unsupported host integration.

## What To Work On If WASM Matters

If you want to push miniR toward a real WASM target, the highest-leverage tasks are:

1. replace or adapt the builtin registry so it does not rely on native linker-section behavior
2. keep parser and evaluator code independent of host-specific features
3. preserve strict feature boundaries around native loading, REPL, GUI, and filesystem-heavy code

The architecture is already trying to help with that. The registry problem is the sharp remaining blocker.
