# Full-Fledged Stacktrace System for miniR

## Context

Error messages like "not all are TRUE (element 1 of argument 1)" are useless without knowing where they came from. The interpreter has `CallFrame` tracking but errors don't include a stack trace. We need a comprehensive system that covers R-level calls, native C code, and eventually source file:line info. This should save 10+ minutes of bisecting per debugging session.

## Current State (already on main, uncommitted)

Layer 1 is done — basic R-level traceback:
- `TraceEntry { call: Option<Expr> }` in `src/interpreter/call.rs`
- `last_traceback: RefCell<Vec<TraceEntry>>` on `Interpreter`
- Captured in `call_closure`/`call_closure_lazy` before frame pop (only first/deepest)
- Cleared at start of `Interpreter::eval()`
- `Session::render_error()` appends traceback to error display
- `traceback()` builtin prints and returns the last traceback
- REPL + CLI use `session.render_error()`

Output currently looks like:
```
Error: not all are TRUE (element 1 of argument 1)
Traceback (most recent call last):
3: stopifnot(x > 0)
2: validate(input)
1: process_data(df)
```

## The 7 Layers

### Layer 1: R-Level Traceback — DONE

No changes needed.

---

### Layer 2: Native C Backtrace Capture

**Goal**: Capture raw C stack frames in `Rf_error()` before `longjmp` destroys them.

**Files**:
- `csrc/native_trampoline.c` — add `backtrace()` call before `longjmp`
- `src/interpreter/native/dll.rs` — read captured frames after `_minir_call_protected` returns error
- `src/interpreter/call.rs` — extend `TraceEntry` with native backtrace

**Changes to trampoline**:
```c
#include <execinfo.h>  // macOS + glibc

#define MAX_BT_FRAMES 64
static void *_bt_frames[MAX_BT_FRAMES];
static int   _bt_count = 0;

// In Rf_error(), before longjmp:
_bt_count = backtrace(_bt_frames, MAX_BT_FRAMES);

// New accessors for Rust:
int _minir_bt_count(void) { return _bt_count; }
void *const *_minir_bt_frames(void) { return _bt_frames; }

// Reset _bt_count = 0 at top of _minir_call_protected()
```

**Changes to dll.rs**: After detecting `error_code != 0`, copy raw addresses into a `Vec<usize>`.

**New data structures**:
```rust
#[derive(Debug, Clone, Default)]
pub struct NativeBacktrace {
    pub frames: Vec<usize>,
}
```

Store `NativeBacktrace` on a `RefCell<Option<NativeBacktrace>>` on `Interpreter` — set by `dot_call`/`dot_c` on error, consumed by `capture_traceback()` and attached to the innermost `TraceEntry`.

**Platform**: `#if defined(__APPLE__) || defined(__GLIBC__)` guard. When unavailable, `_bt_count` stays 0.

**Feature gate**: All behind `#[cfg(feature = "native")]`.

---

### Layer 3: Lightweight Symbol Resolution (dladdr)

**Goal**: Resolve raw addresses → function name + .so path. Zero external deps.

**File**: New `src/interpreter/native/stacktrace.rs`

Use `dladdr()` (libc, available on macOS + Linux) via raw FFI:
```rust
pub struct ResolvedFrame {
    pub address: usize,
    pub function: Option<String>,  // e.g. "my_c_function"
    pub library: Option<String>,   // e.g. "vctrs.dylib"
    pub offset: usize,             // offset from function start
}
```

**Formatting** — native frames render indented under the .Call frame:
```
3: .Call("C_my_function", x, y)
   [C] my_c_function+0x1a (myPkg.dylib)
   [C] helper_func+0x42 (myPkg.dylib)
2: wrapper(x)
1: main_call()
```

Filter out trampoline internals (skip frames until package .so is seen, stop at `_minir_call_protected`).

---

### Layer 4: Debug Symbol Compilation

**Goal**: Compile package C/C++ with `-g` so DWARF debug info exists in .so files.

**Files**:
- `src/interpreter/native/compile.rs` line ~389: add `.debug(true)` and `.flag("-fno-omit-frame-pointer")` to `configure_build`
- `build.rs` line ~8: add `.debug(true)` to the trampoline build

One-line changes. Roughly doubles .o size (negligible for dev interpreter). No runtime perf impact. Immediately improves dladdr symbol names and enables Layer 5.

Optional: `MINIR_STRIP_NATIVE=1` env var to skip `-g` for release/production.

---

### Layer 5: Rich Symbol Resolution (addr2line / gimli)

**Goal**: Resolve addresses → file:line:function using DWARF debug info.

**New deps in `Cargo.toml`**:
```toml
addr2line = { version = "0.26", optional = true, default-features = false, features = ["std"] }
object = { version = "0.36", optional = true, default-features = false, features = ["read", "std"] }
# gimli is transitive via addr2line
```
Gate behind `native` feature.

**File**: `src/interpreter/native/stacktrace.rs` (extend from Layer 3)

Resolution strategy:
1. `dladdr` gets library path + base address for each frame
2. Group frames by library path
3. For each library, lazily create `addr2line::Context` by reading the .so from disk
4. Resolve: `context.find_location(addr - base_addr)` → `(file, line, function)`
5. Cache contexts per library path (thread-local, per-session lifetime)

**Output with DWARF**:
```
3: .Call("C_process_data", x)
   [C] vctrs_init_library at src/init.c:42 (vctrs.dylib)
   [C] Rf_error at native_trampoline.c:38 (miniR)
2: library(vctrs)
1: source("test.R")
```

Falls back to Layer 3 (function name only) when no debug info available.

---

### Layer 6: Interleaved R+C Boundary Markers

**Goal**: When C calls back into R via Rf_eval, show the boundary in the traceback.

**Files**:
- `src/interpreter/call.rs` — add `TraceEntryKind` enum: `RCall`, `NativeCall`, `NativeBoundary`
- `src/interpreter/native/dll.rs` — in `callback_eval_expr()`, push a boundary frame before calling `interp.eval_in()`

**Output**:
```
5: .Call("C_process_data", x)
   --- entered native code ---
4: callback_handler(result)
3: transform(data)
   --- returned to native code ---
2: wrapper(x)
1: main_call()
```

---

### Layer 7: AST Source Spans (Future, Separate Project)

**Goal**: Attach file:line to R-level frames.

**Current state**: `Expr` has no spans. Pest produces spans but `builder.rs` discards them.

**Recommended approach**: Add `span: Option<Span>` only to `Expr::Call` (the variant that appears in tracebacks). Limits blast radius vs wrapping the entire `Expr` type.

```rust
// ast.rs
#[derive(Debug, Clone, Copy)]
pub struct Span { pub start: u32, pub end: u32 }

// Only on Call:
Call { func: Box<Expr>, args: Vec<Arg>, span: Option<Span> },
```

Plus `source_info: RefCell<Option<SourceInfo>>` on `Interpreter` (set by `eval_file`/`source()`), so byte offsets resolve to file:line at format time.

**Output**:
```
3: stopifnot(x > 0) at pillar/R/zzz.R:42
2: assign_crayon_styles() at pillar/R/styles.R:15
1: .onLoad() at pillar/R/zzz.R:3
```

---

## Implementation Order

| Priority | Layer | Effort | Deps | Value |
|----------|-------|--------|------|-------|
| **done** | 1. R-level traceback | done | — | high |
| **1st** | 4. Debug symbols (`-g`) | 2 lines | none | enables everything |
| **2nd** | 2. Native backtrace capture | ~50 LOC C + ~30 LOC Rust | none | foundation |
| **3rd** | 3. dladdr resolution | ~60 LOC Rust | none | immediate readable output |
| **4th** | 5. addr2line/gimli | ~150 LOC Rust | addr2line, object | file:line for C code |
| **5th** | 6. Interleaved frames | ~40 LOC Rust | none | clarity for complex packages |
| **future** | 7. AST spans | large refactor | none | file:line for R code |

Layers 2-4 can be done in a single session. Layer 5 requires vendoring new deps. Layer 6 is independent. Layer 7 is a separate project.

## Key Files

- `csrc/native_trampoline.c` — backtrace capture (Layer 2)
- `build.rs` — trampoline debug symbols (Layer 4)
- `src/interpreter/call.rs` — TraceEntry types, format_traceback (all layers)
- `src/interpreter/native/dll.rs` — read captured backtrace, boundary markers (Layers 2, 6)
- `src/interpreter/native/compile.rs` — `-g` flag (Layer 4)
- `src/interpreter/native/stacktrace.rs` — new module for resolution (Layers 3, 5)
- `src/parser/ast.rs` — Span type (Layer 7)
- `src/parser/builder.rs` — span extraction (Layer 7)
- `Cargo.toml` — addr2line + object deps (Layer 5)

## Verification

- **Layer 1**: `f <- function() stop("boom"); g <- function() f(); g()` → traceback shows `f()` and `g()`
- **Layers 2-3**: Load a native package, trigger Rf_error from C → traceback shows C function names
- **Layer 4-5**: Same as above but with file:line info in the C frames
- **Layer 6**: Package with C→R callbacks → traceback shows boundary markers
- **Layer 7**: `source("script.R")` with error → traceback shows `script.R:42`
