# Rust Extension ABI for miniR

## Why this plan exists

miniR has two native-code paths today:

- a GNU-R-compatible C ABI for CRAN packages (`.Call()`, `dyn.load()`, `Rinternals.h`)
- an internal Rust builtin API for code compiled into the interpreter

Neither is a proper Rust extension system. The question is whether miniR needs
one, and if so, what shape it should take.

## Honest assessment: is this the right time?

**No.** The original plan was 880 lines of detailed design for a system with
zero users. miniR's actual priorities right now are:

1. Running more CRAN packages (71/74 tested, many more untested)
2. Filling out the C API surface for native CRAN code
3. Fixing interpreter correctness (rlang, argument evaluation)
4. The existing 800+ builtins and growing

A Rust extension ABI serves hypothetical third-party Rust package authors who
don't exist yet. Even extendr (Rust extensions for GNU R) has limited adoption,
and a miniR-specific extension system would have even fewer users.

**What this plan does instead**: identify what's worth building now (because it
improves the existing codebase), versus what should wait until there's real demand.

## What the original plan got right

These ideas are sound and should be preserved:

1. **Per-interpreter typed state** — `ctx.state::<T>()` for package-scoped mutable
   state that respects reentrancy. Useful for builtins too, not just extensions.

2. **Typed external objects** — miniR needs `External<T>` for Rust objects that
   survive round-trips through R, with finalizers. This benefits both the C ABI
   path and any future Rust extension path.

3. **The argument decoding model** — `Missing<T>`, `Nullable<T>`, dots handling.
   The existing `FromArgs` / `CoerceArg` traits are a good start but need these
   R-specific distinctions.

4. **Three call levels** — already implemented (Eager, Interpreter, PreEval).
   The original plan correctly identified that extensions need all three.

5. **Descriptor-based registration** — static descriptors are inspectable,
   cacheable, and safe. Already the pattern miniR uses for builtins.

6. **Namespace integration** — extensions should bind into package namespaces
   through the normal package loader, not through ad hoc loading.

## What the original plan got wrong

### 1. Rust `dylib` loading is the worst of both worlds

The original plan proposed loading Rust `dylib` crates via `libloading`. But it
also acknowledged that:

- Rust has no stable ABI
- Extensions must be built with the same rustc version
- Extensions must target the same triple
- Extensions must match the miniR ABI epoch

If you require same-toolchain source builds anyway, **you get all the complexity
of dynamic loading with none of its benefits** (portability, prebuilt binaries,
cross-version compatibility). A `dylib` compiled with rustc 1.85 won't load in
a miniR built with rustc 1.86. Every toolchain update invalidates every cached
extension.

### 2. `linkme` in dylibs is fragile and untested

`linkme` works by exploiting platform-specific linker sections. The plan already
notes that it doesn't work on WASM at all (`plans/linkme-wasm.md`). Using it
inside dynamically loaded libraries adds another fragility layer — different
platforms handle `__DATA` sections in dylibs differently, and the miniR binary's
`BUILTIN_REGISTRY` slice is populated at link time, not at `dlopen` time. The
plan works around this by proposing extension-local slices with a discovery
symbol, but this is untested infrastructure solving a problem that doesn't need
to be solved yet.

### 3. Parallel infrastructure for the same problem

The plan proposes `minir-ext` + `minir-ext-macros` that replicate what
`minir-macros` already does, but targeting a different output. That's two proc
macro crates, two registration systems, two descriptor types, and two code paths
through the evaluator — for a feature with zero users. The maintenance cost is
real even if the code is never used.

### 4. The `Value` wrapper adds indirection without benefit today

Wrapping `RValue` in a public `Value` type to hide internals makes sense when
you have external consumers who need API stability. With zero external consumers,
it's just another layer of boilerplate between the extension author and the
interpreter.

## The right approach: phase by actual need

### Phase 1: Improve the internal extension story (NOW)

These changes benefit the existing codebase immediately. They don't require any
new crates or loading infrastructure.

**1a. Typed external objects**

Add `RValue::External` (or equivalent) for Rust objects that survive R round-trips.
Use the existing `slotmap` dependency for a per-interpreter object table.

```rust
// On Interpreter:
pub fn new_external<T: 'static>(&mut self, value: T) -> ExternalKey { ... }
pub fn get_external<T: 'static>(&self, key: ExternalKey) -> Option<&T> { ... }
pub fn get_external_mut<T: 'static>(&mut self, key: ExternalKey) -> Option<&mut T> { ... }
```

This is useful today for:
- Database connections
- Compiled regex caches
- Graphics device state
- Parser objects
- Any stateful builtin that needs to hand an opaque handle to R

**1b. Per-package state on Interpreter**

Add a typed state map to `Interpreter` so packages (and builtins) can store
per-interpreter mutable state without process globals.

```rust
// On Interpreter:
pub fn package_state<T: Default + 'static>(&mut self) -> &mut T { ... }
```

Internally, a `HashMap<TypeId, Box<dyn Any>>` or similar. This benefits existing
builtins that currently use thread-local state awkwardly.

**1c. Expand `FromArgs` / `CoerceArg`**

The trait system in `value/traits.rs` is the right direction. Extend it:

- `Option<T>` for optional arguments (missing → `None`)
- `Vec<T>` for vector arguments
- `Nullable<T>` wrapper to distinguish `NULL` from missing
- Better error messages that name the function and parameter

**1d. Migrate more builtins to `#[derive(FromArgs)]`**

The `FromArgs` derive is cleaner than raw `args: &[RValue]` unpacking. Use it
more widely. This serves as a proving ground for what a public extension API
would need.

### Phase 2: Public extension crate with static linking (WHEN there are users)

When someone actually wants to write a Rust-native R package for miniR, the
simplest correct approach is **static linking**: the extension crate is a normal
Cargo dependency that registers builtins at compile time.

**How it works:**

1. Package contains `rust/Cargo.toml` depending on `minir-ext`
2. Extension author writes `#[minir::function]` / `#[minir::special]`
3. miniR's package installer adds the crate as a build dependency
4. `cargo build` compiles everything into one binary
5. `linkme` works exactly as it does today — same compilation unit
6. Package loader detects the registered functions and binds them into the namespace

**What this requires:**

- Extract the builtin API surface into a `minir-ext` crate
- `minir-ext-macros` for the proc macros (targeting the extension-local registry)
- A package install step that modifies miniR's build to include the extension
- A mechanism to rebuild miniR with selected extensions

**What this avoids:**

- No dynamic loading
- No ABI validation
- No panic boundaries (same compilation unit)
- No platform-specific symbol resolution
- No `linkme`-in-dylib fragility
- No `Value` wrapper (the crate can re-export `RValue` directly since it's
  compiled together)

**The trade-off is honest:** you must recompile miniR to add Rust extensions.
This is acceptable because:

- miniR is a developer tool, not a production runtime with hot-reload needs
- Compilation is fast (8.5s default, 15s full)
- The R ecosystem already expects source compilation for packages with native code
- If someone needs runtime loading, the C ABI path exists and works

### Phase 3: Dynamic loading (ONLY if static linking proves insufficient)

If Phase 2 reveals a genuine need for runtime-loadable Rust extensions (e.g.,
many users who can't rebuild miniR), the right approach is **not** Rust `dylib`
loading. It's one of:

**Option A: C ABI with Rust ergonomics**

The extension crate compiles as `cdylib` and exports `extern "C"` functions.
Proc macros generate the C thunks automatically from ergonomic Rust code.
Loading goes through the existing `dyn.load()` / `.Call()` infrastructure.

```rust
// Author writes this:
#[minir::function(name = "double_it")]
fn double_it(x: Doubles) -> OwnedDoubles {
    x.iter().map(|v| v.map(|n| n * 2.0)).collect()
}

// Macro generates:
// - An extern "C" fn that converts SEXP args, calls double_it, converts result
// - Registration in R_init_<pkg> for .Call discovery
```

Advantages:
- Stable ABI (C)
- Cross-toolchain compatible
- Uses existing loading infrastructure
- `catch_unwind` happens naturally in the generated thunk
- Battle-tested pattern (this is essentially what extendr does for GNU R)

Disadvantage: thin FFI layer at the boundary. But miniR controls the SEXP
representation, so this can be made very cheap.

**Option B: miniR-specific stable C ABI**

If SEXP conversion overhead is unacceptable, define a miniR-specific C ABI with
opaque handle types that avoid full value copying:

```c
typedef struct minir_value_t* minir_value;
typedef minir_value (*minir_fn)(void* ctx, int argc, minir_value* argv);
```

The handles are indices into interpreter-owned storage, so "conversion" is just
wrapping/unwrapping an integer. This gives C ABI stability with near-zero
overhead.

**Option C: WASM plugins**

Truly portable, sandboxed, stable ABI. Heavy runtime cost. Worth considering
if miniR ever targets environments where native compilation isn't available.

## What NOT to build

- **Don't build `minir-ext` or `minir-ext-macros` until Phase 2.** The internal
  builtin API is still evolving. Premature extraction freezes a moving target.

- **Don't build dylib loading for Rust code.** The C ABI path exists for runtime
  loading. Rust `dylib` is fragile and gives no portability benefit.

- **Don't design for API stability yet.** miniR has no external consumers. The
  internal types (`RValue`, `BuiltinContext`, `Environment`) are the real API.
  Wrapping them adds overhead without benefit.

- **Don't add a second package descriptor type.** The existing `BuiltinDescriptor`
  works. If extensions need more metadata, extend it.

## Implementation order

Phase 1 only. Phase 2 and 3 are deferred until there's demand.

1. Add `ExternalKey` + object table to `Interpreter` using `slotmap`
2. Add `RValue::External(ExternalKey)` variant
3. Add `Interpreter::package_state::<T>()` typed state map
4. Add `CoerceArg` impls for `Option<T>`, `Vec<f64>`, `Vec<String>`
5. Add `Nullable<T>` wrapper type to `FromArgs`
6. Migrate 5-10 existing builtins to `#[derive(FromArgs)]` as a proving ground
7. Add external object finalizer support (run on interpreter drop)

Each of these is a standalone commit that improves the existing codebase. None
requires new crates, new loading infrastructure, or new macro systems.

## Open questions for Phase 2 (record now, decide later)

- Should the extension crate re-export `RValue` directly or wrap it?
  (Depends on how stable `RValue` is when Phase 2 starts.)
- Should extensions use the same macro crate as builtins, or a separate one?
  (Depends on whether internal/external registration needs diverge.)
- Should `minir install.packages()` trigger a miniR rebuild automatically?
  (UX question — convenient but surprising.)
- Is the C ABI thunk approach (Phase 3, Option A) fast enough for hot-path
  extensions? (Benchmark when there's a real use case.)
