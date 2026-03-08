# Rust std → R Mapping & Opportunities

Mapping every Rust std module to its R equivalent (if any), then identifying
things Rust std offers that R does NOT have natively — potential extensions for newr.

## Module-by-module mapping

### Already mapped to R equivalents

| Rust std module | R equivalent | Notes |
|---|---|---|
| `str` / `string` | `character` vectors, `paste`, `substr`, `gsub`, `grep`, `strsplit`, `nchar`, `sprintf` | R has extensive string support |
| `vec` | `c()`, vectors | R's fundamental type |
| `collections::HashMap` | named lists, `environment()` | R lists with names act as hash maps |
| `collections::BTreeMap` | (none — sorted named list) | R has no sorted map |
| `collections::HashSet` | `unique()`, `union()`, `intersect()`, `setdiff()` | R does sets via vectors |
| `collections::VecDeque` | (none) | No double-ended queue in base R |
| `collections::BinaryHeap` | (none) | No priority queue in base R |
| `collections::LinkedList` | (none) | No linked list in base R |
| `option` | `NULL` / `NA` | R uses NA for missing values, NULL for absence |
| `result` | `tryCatch()` / `try()` | R uses condition system instead of Result |
| `error` | `condition`, `simpleError`, `simpleWarning` | R's condition system |
| `fmt` | `format()`, `sprintf()`, `formatC()`, `cat()`, `print()` | R has rich formatting |
| `io` | `readLines()`, `writeLines()`, `scan()`, `cat()`, `connections` | R's connection system |
| `fs` | `file.create()`, `file.copy()`, `file.remove()`, `dir.create()`, `list.files()`, `file.info()` | R has full file ops |
| `path` | `file.path()`, `basename()`, `dirname()`, `normalizePath()`, `path.expand()` | R has basic path ops |
| `env` | `Sys.getenv()`, `Sys.setenv()` | Direct equivalent |
| `process` | `system()`, `system2()`, `processx` package | R can spawn processes |
| `net` | `url()`, `download.file()`, `socketConnection()`, `httr`/`curl` packages | R has basic networking |
| `thread` | `parallel` package, `mclapply()`, `future` package | R has parallelism but no threads |
| `sync` | (none) | R is single-threaded (GIL equivalent) |
| `time` | `Sys.time()`, `proc.time()`, `system.time()`, `difftime` | R has time support |
| `iter` | `sapply`, `lapply`, `Map`, `Filter`, `Reduce`, `for` loops | R's apply family |
| `cmp` | `sort()`, `order()`, `rank()`, comparison operators | R has rich comparison |
| `clone` | `<-` (copy-on-modify semantics) | R copies by default |
| `convert` | `as.numeric()`, `as.character()`, `as.integer()`, `as.logical()` | R coercion |
| `num` | numeric types, `is.finite()`, `is.nan()`, `is.infinite()` | R has these |
| `f64` / `f32` | `double` (R only has f64) | R is double-precision only |
| `i32` / `i64` / etc. | `integer` (R only has i32) | R integers are 32-bit |
| `hash` | `digest` package | Not in base R |
| `marker` | (N/A) | Type system concept, no R equivalent |
| `mem` | `object.size()`, `gc()`, `mem.limits()` | Basic memory info |
| `ops` | `+`, `-`, `*`, `/`, `%%`, `%/%`, `^`, operators | R has rich operators |
| `slice` | vector indexing `x[1:5]`, `head()`, `tail()` | R's core strength |
| `array` | `array()`, `matrix()` | R has arrays natively |
| `char` | (part of character) | R doesn't have a char type |
| `ascii` | `chartr()`, `toupper()`, `tolower()` | Partial |
| `borrow` | (N/A) | Ownership concept, not applicable |
| `cell` | (N/A) | Interior mutability, not applicable |
| `pin` / `ptr` / `rc` | (N/A) | Memory management, not applicable |
| `future` / `task` / `async_iter` | `promises` package, `future` package | Not in base R |
| `panic` / `backtrace` | `tryCatch()`, `traceback()`, `sys.calls()` | R has stack traces |
| `random` | `runif()`, `rnorm()`, `sample()`, `set.seed()` | R has excellent RNG |
| `simd` | (none) | No SIMD in R |
| `default` | default argument values | R has defaults in function signatures |
| `range` | `1:10`, `seq()` | R has range generation |
| `any` | `is()`, `class()`, `typeof()` | R's type introspection |
| `hint` | (N/A) | Compiler hints, not applicable |
| `ffi` | `.Call()`, `.C()`, `.External()` | R has FFI |
| `prelude` | base package auto-attached | R's base is always available |
| `os` | `.Platform`, `Sys.info()`, `R.version` | Platform info |

### NOT in R — potential newr extensions

These are capabilities from Rust's std that R lacks entirely. These represent
opportunities for newr to go beyond standard R.

#### 1. `collections::BTreeMap` — Sorted map
R has no ordered/sorted associative container. Could add:
```r
sorted_map()          # create sorted map
sorted_map_insert(m, key, value)
sorted_map_keys(m)    # returns keys in sorted order
# or just: sm <- sorted_list(); sm[["key"]] <- value with sorted iteration
```
**Priority: Low** — niche use case

#### 2. `collections::VecDeque` — Double-ended queue
R has no deque. Appending/prepending to vectors is O(n). Could add:
```r
deque()               # create deque
deque_push_front(d, x)
deque_push_back(d, x)
deque_pop_front(d)
deque_pop_back(d)
```
**Priority: Medium** — useful for BFS, sliding windows, job queues

#### 3. `collections::BinaryHeap` — Priority queue
No priority queue in base R. Could add:
```r
heap()                # create min/max heap
heap_push(h, x)
heap_pop(h)           # returns min/max
heap_peek(h)
```
**Priority: Medium** — useful for algorithms, scheduling

#### 4. `thread` / `sync` — Real parallelism
R's `parallel` package uses forking (Unix) or socket clusters. newr could offer:
```r
thread(expr)          # spawn OS thread
channel()             # create message-passing channel
send(ch, value)
recv(ch)
mutex(value)          # create mutex-protected value
atomic(value)         # atomic counter
```
**Priority: High** — R's biggest pain point. True shared-memory parallelism
would be transformative. Rust makes this safe.

#### 5. `net` — Built-in async networking
R's networking is blocking and limited. Could add:
```r
tcp_connect(host, port)
tcp_listen(port)
tcp_read(conn)
tcp_write(conn, data)
udp_socket(port)
```
**Priority: Low** — packages handle this, but built-in would be cleaner

#### 6. `hash` — Built-in hashing
R has no native hash function. Could add:
```r
hash(x)               # returns integer hash of any R value
hash_bytes(raw_vec)    # hash raw bytes
```
**Priority: Medium** — useful for deduplication, caching, consistent hashing

#### 7. `fs` — Richer file system operations
R's file ops lack some things Rust has:
```r
file.metadata(path)       # returns full metadata (permissions, timestamps, size, type)
file.read_bytes(path)     # read file as raw vector (not text)
file.write_bytes(path, raw)
dir.walk(path)            # recursive directory iterator (like fs::read_dir recursive)
file.canonicalize(path)   # true canonical path (resolving all symlinks)
file.symlink(target, link)
file.hardlink(target, link)
```
**Priority: Low** — R handles most cases, but `dir.walk` would be nice

#### 8. `time::Instant` — Monotonic high-resolution timer
R's `proc.time()` works but is clunky. Could add:
```r
timer_start()          # returns opaque instant
timer_elapsed(t)       # returns seconds since instant (monotonic, not wall-clock)
```
**Priority: Low** — `system.time()` mostly covers this

#### 9. `simd` — Vectorized low-level operations
R vectors are already implicitly vectorized, but actual SIMD could speed up:
```r
# No new API needed — just use SIMD internally for:
# - vector arithmetic (+, -, *, /)
# - comparison operations
# - sum(), prod(), cumsum()
# This is an implementation detail, not a user-facing feature
```
**Priority: High (internal)** — huge performance win, no API change needed

#### 10. `Result`-style error handling
R's tryCatch is verbose. Could add a Rust-inspired pattern:
```r
result <- try_result(expr)  # returns list(ok=value) or list(err=condition)
unwrap(result)              # returns value or stops
unwrap_or(result, default)  # returns value or default
is_ok(result)
is_err(result)
# or pipe-friendly:
expr |> try_result() |> unwrap_or(NA)
```
**Priority: Medium** — ergonomic improvement over tryCatch

#### 11. `iter` — Lazy iterators
R evaluates everything eagerly. Rust's iterator model could inspire:
```r
iter(1:1000000)          # lazy iterator (doesn't allocate)
iter_map(it, fn)         # lazy map
iter_filter(it, fn)      # lazy filter
iter_take(it, n)         # take first n
iter_collect(it)         # materialize to vector
iter_chain(it1, it2)     # concatenate iterators
iter_zip(it1, it2)       # zip two iterators
iter_enumerate(it)       # (index, value) pairs
```
**Priority: High** — memory-efficient processing of large data without
materializing intermediate vectors. R's biggest memory problem.

#### 12. Pattern matching
Rust's `match` is more powerful than R's `switch`:
```r
match_expr(x,
  is.numeric ~ sqrt(x),
  is.character ~ nchar(x),
  is.null ~ 0,
  _ ~ stop("unknown type")
)
```
**Priority: Medium** — switch() is limited, real pattern matching would help

## Summary: Top opportunities for newr beyond R

Ranked by impact:

1. **Lazy iterators** — memory-efficient pipelines without intermediate allocation
2. **True parallelism** — threads + channels + mutex, leveraging Rust's safety
3. **SIMD internals** — transparent speedup of vector operations
4. **Priority queue / deque** — data structures R lacks
5. **Result-style errors** — ergonomic error handling
6. **Built-in hashing** — `hash()` on any value
7. **Pattern matching** — richer than `switch()`
