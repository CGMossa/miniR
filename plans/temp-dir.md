# temp-dir integration plan

> `temp-dir` 0.2 — Zero-dependency temporary directory with auto-cleanup on drop.
> https://lib.rs/crates/temp-dir
> Apache-2.0, zero deps, zero unsafe.

## What it does

`TempDir` creates a temporary directory in the system temp location. When the `TempDir` value is dropped, the directory and all its contents are recursively deleted. No dependencies — uses only `std::fs`.

### Key API

```rust
use temp_dir::TempDir;

let dir = TempDir::new().unwrap();
let path = dir.path();                    // &Path to the temp dir
let child = dir.child("myfile.txt");      // PathBuf inside the dir

// dir is deleted when it goes out of scope
// unless:
dir.dont_delete_on_drop();  // keep it around
```

## Where it fits in newr

### Current `tempdir()` and `tempfile()` implementations

Our current builtins (in `builtins/system.rs`) use a DIY approach:

- `tempdir()` — just returns `std::env::temp_dir()` (the system temp, not a unique subdir)
- `tempfile()` — constructs a path with `{pid}{nanoseconds}` but never creates the file or dir

Problems:
- `tempdir()` returns the shared system temp dir, not a unique per-session directory like R does
- `tempfile()` paths could collide in theory (timestamp-based)
- No cleanup of temp files on interpreter exit

### With temp-dir

```rust
use temp_dir::TempDir;

// In the Interpreter struct — session-scoped temp dir
pub struct Interpreter {
    // ...
    temp_dir: TempDir,  // auto-cleaned on interpreter drop
}

impl Interpreter {
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        // ...
    }
}

// tempdir() builtin — returns path to session temp dir
fn builtin_tempdir() -> RResult<RValue> {
    with_interpreter(|interp| {
        let path = interp.temp_dir.path().to_string_lossy().to_string();
        Ok(RValue::vec(Vector::Character(vec![Some(path)].into())))
    })
}

// tempfile() builtin — creates unique file path inside session temp dir
fn builtin_tempfile(args, named) -> RResult<RValue> {
    with_interpreter(|interp| {
        let pattern = /* extract pattern arg, default "file" */;
        let fileext = /* extract fileext arg, default "" */;
        let base = interp.temp_dir.path();
        let name = format!("{}{}{}", pattern, unique_id(), fileext);
        let path = base.join(name).to_string_lossy().to_string();
        Ok(RValue::vec(Vector::Character(vec![Some(path)].into())))
    })
}
```

### Benefits over current approach

| | Current | With temp-dir |
|---|---|---|
| `tempdir()` returns | System temp (shared) | Unique session dir |
| Cleanup on exit | None | Automatic (Drop) |
| Collision risk | Possible | None (unique dir) |
| Dependencies | None | Zero-dep crate |

## Implementation order

1. Add `temp-dir = "0.2"` to Cargo.toml, vendor it
2. Add `TempDir` field to `Interpreter` struct
3. Rewrite `tempdir()` and `tempfile()` as interpreter builtins (need `with_interpreter`)
4. Remove the old implementations from `builtins/system.rs`

## Priority

Medium — fixes real correctness issues (shared tempdir, no cleanup). Zero deps, trivial to integrate.
