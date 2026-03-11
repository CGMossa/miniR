# Standard Builtins Implementation Plan

Mapping every missing/stub R builtin to the Rust std module that powers its implementation.
Organized by Rust std module → R functions it enables.

API details sourced from `rustup doc std::*` (Rust 1.93.0).

## Current stats

- **Real implementations:** ~150 functions
- **Partial:** ~15 functions (regex is literal-only, matrices limited, etc.)
- **Stubs/noops:** ~105 functions (return first arg or NULL)
- **Not registered at all:** ~40 known missing functions

---

## Phase 1 — `std::f64` / `std::num` → Numeric builtins

> `rustup doc f64` — 64-bit floating-point primitive type.

### Available f64 methods (from rustdoc)

Math: `floor`, `ceil`, `round`, `round_ties_even`, `trunc`, `fract`,
`abs`, `signum`, `copysign`, `mul_add`, `div_euclid`, `rem_euclid`,
`powi`, `powf`, `sqrt`, `cbrt`, `hypot`, `exp`, `exp2`, `exp_m1`,
`ln`, `log`, `log2`, `log10`, `ln_1p`

Trig: `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `sin_cos`,
`sinh`, `cosh`, `tanh`, `asinh`, `acosh`, `atanh`

Special: `gamma`, `ln_gamma`, `erf`, `erfc`

Comparison: `max`, `min`, `maximum`, `minimum`, `midpoint`, `clamp`, `total_cmp`

Classification: `is_nan`, `is_infinite`, `is_finite`, `is_subnormal`, `is_normal`,
`is_sign_positive`, `is_sign_negative`, `classify`

Conversion: `to_degrees`, `to_radians`, `recip`, `to_bits`, `from_bits`

### Constants (`f64::consts`)

`PI`, `TAU`, `E`, `SQRT_2`, `LN_2`, `LN_10`, `LOG2_E`, `LOG2_10`,
`LOG10_E`, `LOG10_2`, `FRAC_PI_2`, `FRAC_PI_3`, `FRAC_PI_4`, `FRAC_PI_6`,
`FRAC_PI_8`, `FRAC_1_PI`, `FRAC_2_PI`, `FRAC_2_SQRT_PI`, `FRAC_1_SQRT_2`

### Already done

- `abs`, `sqrt`, `exp`, `log`, `log2`, `log10`, `ceiling`, `floor`, `trunc`
- `sin`, `cos`, `tan`, `sign`, `round`
- `sum`, `prod`, `max`, `min`, `mean`, `median`, `var`, `sd`
- `cumsum`, `cumprod`, `cummax`, `cummin`
- `is.finite`, `is.infinite`, `is.nan`, `is.na`
- `Inf`, `NaN`, `NA`, `pi`

### Still missing (easy, implement with f64 methods)

| R function | Rust impl | Priority |
|---|---|---|
| `asin(x)` | `f64::asin()` | high |
| `acos(x)` | `f64::acos()` | high |
| `atan(x)` | `f64::atan()` | high |
| `atan2(y, x)` | `f64::atan2()` | high |
| `sinh(x)`, `cosh(x)`, `tanh(x)` | `f64::sinh()`, etc. | medium |
| `asinh(x)`, `acosh(x)`, `atanh(x)` | `f64::asinh()`, etc. | medium |
| `expm1(x)` | `f64::exp_m1()` | low |
| `log1p(x)` | `f64::ln_1p()` | low |
| `gamma(x)`, `lgamma(x)` | `f64::gamma()` (nightly) or manual | medium |
| `digamma(x)`, `trigamma(x)` | manual implementation | low |
| `beta(a,b)`, `lbeta(a,b)` | from gamma | low |
| `factorial(n)` | `(1..=n).product()` | medium |
| `choose(n,k)` | binomial coefficient | medium |
| `signif(x, digits)` | rounding to significant digits | medium |
| `pmin(...)` / `pmax(...)` | element-wise min/max | high |
| `cumall(x)` / `cumany(x)` | cumulative logical AND/OR | low |
| `complex(...)` | needs Complex type | defer |

### Effort: ~2 sessions for trig/hyp, 1 session for pmin/pmax + factorial/choose

---

## Phase 2 — `std::string` / `std::str` → String builtins

> `rustup doc std::str` — Utilities for the `str` primitive type.
> `rustup doc std::string` — A UTF-8–encoded, growable string.

### Available str methods (from rustdoc)

Searching: `contains`, `starts_with`, `ends_with`, `find`, `rfind`,
`matches`, `rmatches`, `match_indices`, `rmatch_indices`

Splitting: `split`, `rsplit`, `splitn`, `rsplitn`, `split_whitespace`,
`split_ascii_whitespace`, `split_terminator`, `lines`

Transforming: `replace`, `replacen`, `to_uppercase`, `to_lowercase`,
`to_ascii_uppercase`, `to_ascii_lowercase`, `repeat`

Trimming: `trim`, `trim_start`, `trim_end`, `trim_matches`,
`trim_start_matches`, `trim_end_matches`, `strip_prefix`, `strip_suffix`

Conversion: `parse`, `chars`, `char_indices`, `bytes`, `as_bytes`,
`encode_utf16`, `is_ascii`, `is_empty`, `len`

Iterators: `Bytes`, `Chars`, `CharIndices`, `Lines`, `SplitWhitespace`

String: `String::new`, `String::from`, `String::push_str`, `String::push`,
`String::from_utf8`, `String::from_utf16`, `format!`

### Already done

- `nchar`, `substr`, `substring`, `toupper`, `tolower`, `trimws`
- `paste`, `paste0`, `sprintf` (partial), `format`
- `strsplit`, `startsWith`, `endsWith`, `chartr`
- `gsub`, `sub`, `grep`, `grepl` (literal only — no regex)
- `make.names`, `make.unique`, `deparse`, `strtoi`, `nzchar`
- `sQuote`, `dQuote`, `basename`, `dirname`

### Still missing

| R function | Rust impl | Priority |
|---|---|---|
| `formatC(x, width, format, flag)` | `format!` with padding | medium |
| `strrep(x, times)` | `str::repeat()` | high |
| `regexpr()` | needs `regex` crate | high (Phase 5) |
| `gregexpr()` | needs `regex` crate | high (Phase 5) |
| `regmatches()` | needs `regex` crate | high (Phase 5) |
| `regexec()` | needs `regex` crate | medium |
| `glob2rx()` | string transform | low |
| `intToUtf8(x)` | `char::from_u32()` + `String` | medium |
| `utf8ToInt(x)` | `str::chars()` → `u32` | medium |
| `charToRaw(x)` | `str::as_bytes()` | low |
| `rawToChar(x)` | `String::from_utf8()` | low |
| `sprintf()` — full | handle `%0Xd`, `%-s`, `%+f`, `*` width | medium |
| `encodeString()` | escape special chars | low |

### Effort: 1 session for strrep + formatC + utf8, regex is Phase 5

---

## Phase 3 — `std::collections` → Data structure builtins

> `rustup doc std::collections` — Collection types.

Sequences: `Vec`, `VecDeque`, `LinkedList`
Maps: `HashMap`, `BTreeMap`
Sets: `HashSet`, `BTreeSet`
Misc: `BinaryHeap`

R's vectors are already `Vec<Option<T>>`. Lists are `Vec<(Option<String>, RValue)>`.

### Already done

- `c()`, `list()`, `vector()`, `append()`, `rev()`, `sort()`, `order()`
- `unique()`, `duplicated()`, `match()`, `which()`, `which.min()`, `which.max()`
- `head()`, `tail()`, `rep()`, `rep_len()`, `rep.int()`, `seq()`, `seq_len()`, `seq_along()`
- `setdiff()`, `intersect()`, `union()`
- `unlist()`, `as.list()`
- `names()`, `length()`

### Still missing

| R function | Rust impl | Priority |
|---|---|---|
| `rank(x)` | sort indices → rank | medium |
| `tabulate(bin, nbins)` | count occurrences in vec | medium |
| `table(...)` | cross-tabulation via HashMap | medium |
| `factor(x, levels)` | integer codes + levels attr | high |
| `levels(x)` / `nlevels(x)` | attr get | high (with factor) |
| `array(data, dim)` | vector + dim attr | medium |
| `rbind(...)` / `cbind(...)` | matrix concat | medium |
| `diag(x)` | diagonal extraction/creation | low |
| `outer(X, Y, FUN)` | Cartesian product | low |

### Effort: 1 session for factor, 1 session for table/tabulate/rank

---

## Phase 4 — `std::collections::HashMap` → Environment & lookup builtins

> `rustup doc std::collections::HashMap` — A hash map implemented with
> quadratic probing and SIMD lookup (SwissTable).

Key methods: `get`, `get_mut`, `insert`, `remove`, `contains_key`,
`keys`, `values`, `iter`, `len`, `is_empty`, `entry`, `retain`

Our `Environment` already wraps `HashMap<String, RValue>` in `Rc<RefCell<>>`.

### Already done

- `get()`, `assign()`, `exists()` (interpreter builtins)
- `Sys.getenv()`
- `ls()` — partially (via environment)
- `environment()`, `new.env()` — stubs

### Still missing

| R function | Rust impl | Priority |
|---|---|---|
| `ls(envir)` | `env.bindings.keys()` | high |
| `rm(list, envir)` | `HashMap::remove()` | medium |
| `environment(fun)` | return closure env | high |
| `environment(fun) <- env` | set closure env | medium |
| `new.env(parent)` | `Environment::new_child()` | high |
| `globalenv()` | return global env ref | high |
| `baseenv()` | return base env ref | medium |
| `parent.env(env)` | `env.parent()` | medium |
| `parent.frame(n)` | call stack env | medium |
| `environmentName(env)` | env name string | low |
| `as.environment(x)` | coerce to env | low |

### Effort: 1 session for ls/rm/environment/new.env/globalenv

---

## Phase 5 — `regex` crate → Pattern matching builtins

Not std, but the `regex` crate is the standard Rust solution. Currently gsub/sub/grep/grepl
do literal substring matching only.

### Need to upgrade to real regex

| R function | Current | Needed |
|---|---|---|
| `grep(pattern, x)` | literal `str::contains` | `Regex::is_match` |
| `grepl(pattern, x)` | literal `str::contains` | `Regex::is_match` |
| `sub(pattern, repl, x)` | literal `str::replacen(1)` | `Regex::replacen(1)` |
| `gsub(pattern, repl, x)` | literal `str::replace` | `Regex::replace_all` |
| `regexpr(pattern, x)` | noop | `Regex::find` → match pos + length |
| `gregexpr(pattern, x)` | noop | `Regex::find_iter` → all positions |
| `regmatches(x, m)` | noop | extract by positions from regexpr |
| `regexec(pattern, x)` | noop | `Regex::captures` → groups |

Parameters to handle:

- `fixed = TRUE` → literal matching (current behavior)
- `ignore.case = TRUE` → `RegexBuilder::case_insensitive(true)`
- `perl = TRUE` → default in modern R, use `regex` crate
- `value = TRUE` (grep) → return matching elements instead of indices

### Effort: 1 session — add `regex` dep, upgrade grep/grepl/sub/gsub, implement regexpr/gregexpr

---

## Phase 6 — `std::fs` / `std::path` / `std::env` → File I/O builtins

> `rustup doc std::fs` — Filesystem manipulation operations.
> `rustup doc std::path` — Cross-platform path manipulation.
> `rustup doc std::env` — Inspection and manipulation of the process's environment.

### std::fs functions (from rustdoc)

`canonicalize`, `copy`, `create_dir`, `create_dir_all`, `exists`,
`hard_link`, `metadata`, `read`, `read_dir`, `read_link`,
`read_to_string`, `remove_dir`, `remove_dir_all`, `remove_file`,
`rename`, `set_permissions`, `symlink_metadata`, `write`

Structs: `File`, `DirEntry`, `ReadDir`, `Metadata`, `Permissions`,
`FileType`, `OpenOptions`, `DirBuilder`

### std::path types

`Path` — immutable path reference (like `&str` for paths)
`PathBuf` — owned path (like `String` for paths)

Methods: `exists`, `is_file`, `is_dir`, `canonicalize`, `parent`,
`file_name`, `file_stem`, `extension`, `join`, `with_extension`,
`components`, `starts_with`, `ends_with`, `display`

### std::env functions (from rustdoc)

`current_dir`, `set_current_dir`, `current_exe`, `home_dir`,
`temp_dir`, `var`, `var_os`, `set_var`, `remove_var`,
`vars`, `args`, `split_paths`, `join_paths`

Constants: `std::env::consts::OS`, `ARCH`, `FAMILY`, `EXE_SUFFIX`

### Already done

- `file.exists()` → `Path::exists()`
- `file.path()` → `Path::join()`
- `readLines()` → `fs::read_to_string` + split
- `writeLines()` → `fs::write`
- `basename()` / `dirname()` → `Path::file_name()` / `Path::parent()`
- `getwd()` — stub (returns ".")

### Still missing

| R function | Rust impl | Priority |
|---|---|---|
| `getwd()` — real | `std::env::current_dir()` | high |
| `setwd(dir)` | `std::env::set_current_dir()` | high |
| `dir.exists(path)` | `Path::is_dir()` | high |
| `dir.create(path)` | `fs::create_dir_all()` | medium |
| `list.files(path)` / `dir()` | `fs::read_dir()` | high |
| `file.info(path)` | `fs::metadata()` → list | medium |
| `file.size(path)` | `fs::metadata().len()` | medium |
| `file.copy(from, to)` | `fs::copy()` | medium |
| `file.rename(from, to)` | `fs::rename()` | medium |
| `file.remove(path)` | `fs::remove_file()` | medium |
| `file.create(path)` | `File::create()` | low |
| `unlink(path)` | `fs::remove_file` / `remove_dir_all` | medium |
| `tempfile()` | `std::env::temp_dir()` + random name | medium |
| `tempdir()` | `std::env::temp_dir()` | medium |
| `normalizePath(path)` | `Path::canonicalize()` | medium |
| `path.expand(path)` | expand `~` via env | medium |
| `Sys.glob(paths)` | `glob` crate or manual | low |
| `scan(file)` | parse whitespace-separated values | low |
| `source(file)` | already done (interpreter builtin) | done |

### Effort: 1 session for the common ones (getwd, setwd, list.files, dir.exists, file.*)

---

## Phase 7 — `std::time` → Date/time builtins

> `rustup doc std::time` — Temporal quantification.

Structs: `Duration`, `Instant`, `SystemTime`, `SystemTimeError`

`Duration` — span of time: `from_secs`, `from_millis`, `from_micros`,
`from_nanos`, `from_secs_f64`, `as_secs`, `as_secs_f64`

`Instant` — monotonically nondecreasing clock:
`now()`, `elapsed()`, `duration_since()`, `checked_add()`, `checked_sub()`

`SystemTime` — wall-clock time:
`now()`, `elapsed()`, `duration_since(UNIX_EPOCH)`

Constants: `UNIX_EPOCH`

### Already done

- `Sys.time()` → `SystemTime::now()` as seconds since epoch
- `Sys.sleep(n)` → `thread::sleep(Duration::from_secs_f64(n))`
- `proc.time()` → stub (returns zeros)

### Still missing

| R function | Rust impl | Priority |
|---|---|---|
| `proc.time()` — real | `Instant::now()` at startup, elapsed | high |
| `system.time(expr)` — real | `Instant::now()` before/after eval | high |
| `difftime(t1, t2)` | subtraction of numeric times | medium |
| `as.POSIXct(x)` | parse datetime string → epoch seconds | low |
| `as.POSIXlt(x)` | parse → broken-down time (list) | low |
| `strptime(x, format)` | parse with format string | low |
| `strftime(x, format)` | format epoch → string | low |
| `Sys.timezone()` | env var or system call | low |
| `date()` | formatted current date string | medium |
| `as.Date(x)` | days since epoch | low |

### Effort: 1 quick session for proc.time + system.time; datetime parsing is a bigger project

---

## Phase 8 — `std::process` → System builtins

> `rustup doc std::process` — A module for working with processes.

`Command` — configure and spawn: `new(program)`, `arg(a)`, `args(list)`,
`env(key, val)`, `current_dir(path)`, `stdin(cfg)`, `stdout(cfg)`,
`stderr(cfg)`, `spawn()`, `output()`, `status()`

`Output` — result: `stdout: Vec<u8>`, `stderr: Vec<u8>`, `status: ExitStatus`

`Stdio` — pipe config: `piped()`, `inherit()`, `null()`

Functions: `exit(code)`, `abort()`, `id()`

### Already done

- `q()` / `quit()` → `process::exit(0)`

### Still missing

| R function | Rust impl | Priority |
|---|---|---|
| `system(command)` | `Command::new("sh").arg("-c").arg(cmd).output()` | high |
| `system2(command, args)` | `Command::new(cmd).args(args).output()` | high |
| `Sys.which(name)` | `Command::new("which").arg(name)` or `PATH` search | low |
| `Sys.setenv(...)` | `std::env::set_var()` | medium |
| `Sys.info()` | `std::env::consts::OS`, `ARCH`, etc. | medium |
| `.Platform` | `std::env::consts::*` | medium |

### Effort: 1 quick session for system/system2/Sys.setenv

---

## Phase 9 — `std::random` → Random number builtins

> `rustup doc std::random` — Random value generation. (nightly-only experimental API)

`random::<f64>()` — generate random f64 in [0, 1)
`DefaultRandomSource` — the default random source

Note: `std::random` is still unstable (#130703). For stable Rust, either
use `rand` crate or implement a simple RNG (xoshiro256, PCG) inline.

### Currently noop

| R function | Rust impl | Priority |
|---|---|---|
| `runif(n, min, max)` | `random::random::<f64>()` scaled | high |
| `rnorm(n, mean, sd)` | Box-Muller or Ziggurat | high |
| `rbinom(n, size, prob)` | binomial sampling | medium |
| `sample(x, size, replace)` | Fisher-Yates shuffle / selection | high |
| `set.seed(seed)` | seed a local RNG | high |
| `rpois(n, lambda)` | Poisson sampling | low |
| `rexp(n, rate)` | exponential: `-ln(U)/rate` | low |
| `rhyper(nn, m, n, k)` | hypergeometric | low |

### Effort: 1 session for runif + rnorm + sample + set.seed

---

## Phase 10 — `std::hash` → Hashing builtins (miniR extension)

> `rustup doc std::hash` — Generic hashing support.

Traits: `Hash`, `Hasher`, `BuildHasher`
Struct: `DefaultHasher` (SipHash 1-3)

Usage: `#[derive(Hash)]` or impl `Hash` manually, then feed to a `Hasher`
and call `finish()` to get `u64`.

Not standard R, but useful:

| R function | Rust impl | Priority |
|---|---|---|
| `hash(x)` | `DefaultHasher` on any RValue | low |
| `digest(x, algo)` | SHA-256, MD5 via crate | low |

### Effort: small, but low priority

---

## Phase 11 — `std::io` → I/O builtins

> `rustup doc std::io` — Traits, helpers, and type definitions for core I/O.

Core traits: `Read`, `Write`, `Seek`, `BufRead`

Structs: `BufReader`, `BufWriter`, `Cursor`, `Stdin`, `Stdout`, `Stderr`,
`Take`, `Chain`, `Bytes`, `Lines`

Functions: `stdin()`, `stdout()`, `stderr()`, `copy(reader, writer)`

`BufRead::lines()` → iterator of `Result<String>` (line-by-line reading)
`BufRead::read_line(&mut buf)` → read one line into String
`Read::read_to_string(&mut buf)` → slurp entire contents

### Already done

- `cat()` → stdout write
- `message()` → stderr write
- `readline()` → stdin read
- `readLines()` → file read
- `writeLines()` → file write

### Still missing

| R function | Rust impl | Priority |
|---|---|---|
| `readRDS(file)` | custom serialization | low |
| `saveRDS(obj, file)` | custom serialization | low |
| `read.csv(file)` | BufReader + split on comma | high |
| `write.csv(x, file)` | format + write | high |
| `read.table(file)` | BufReader + whitespace split | medium |
| `write.table(x, file)` | format + write | medium |
| `sink(file)` | redirect stdout | low |
| `connection()` / `open()` / `close()` | `File` + handle tracking | low |
| `url(x)` | needs HTTP crate | defer |

### Effort: 1 session for read.csv/write.csv

---

## Phase 12 — `std::iter` → Apply family & functional builtins

> `rustup doc std::iter` — Composable external iteration.

Core trait: `Iterator` with `Item` type and `next()` method.

Key adapters: `map`, `filter`, `filter_map`, `flat_map`, `flatten`,
`zip`, `enumerate`, `chain`, `take`, `skip`, `take_while`, `skip_while`,
`peekable`, `step_by`, `inspect`, `rev`, `cycle`, `cloned`, `copied`

Consumers: `collect`, `fold`, `reduce`, `for_each`, `count`, `sum`,
`product`, `min`, `max`, `min_by`, `max_by`, `any`, `all`, `find`,
`find_map`, `position`, `nth`, `last`, `unzip`, `partition`

Three forms of iteration: `iter()` (&T), `iter_mut()` (&mut T),
`into_iter()` (T).

### Already done

- `sapply()`, `lapply()`, `vapply()` — interpreter builtins
- `Reduce()`, `Filter()`, `Map()` — interpreter builtins
- `any()`, `all()` — regular builtins

### Still missing

| R function | Rust impl | Priority |
|---|---|---|
| `apply(X, MARGIN, FUN)` | iterate rows/cols of matrix | medium |
| `mapply(FUN, ...)` | zip multiple lists + map | medium |
| `tapply(X, INDEX, FUN)` | group-by via HashMap + apply | medium |
| `by(data, INDICES, FUN)` | similar to tapply | low |
| `Vectorize(FUN)` | wrap fn to accept vector args | low |
| `Position(f, x)` | `iter().position()` | low |
| `Find(f, x)` | `iter().find()` | low |
| `Negate(f)` | return `!f(x)` wrapper | low |

### Effort: 1 session for apply/mapply/tapply (need matrix support first)

---

## Phase 13 — `std::cmp` / `std::ops` → Comparison & operator builtins

> `rustup doc std::cmp` — Utilities for comparing and ordering values.

Traits: `PartialEq`, `Eq`, `PartialOrd`, `Ord`
Enum: `Ordering` (Less, Equal, Greater)
Struct: `Reverse` (reverse ordering wrapper)
Functions: `min`, `max`, `min_by`, `max_by`, `min_by_key`, `max_by_key`,
`minmax` (experimental)

### Already done

- All comparison operators: `==`, `!=`, `<`, `>`, `<=`, `>=`
- All arithmetic operators: `+`, `-`, `*`, `/`, `^`, `%%`, `%/%`
- `sort()`, `order()`, `rank()` (sort/order done, rank missing)
- `range()`, `diff()`

### Still missing

| R function | Rust impl | Priority |
|---|---|---|
| `rank(x)` | sort + invert permutation | medium |
| `xtfrm(x)` | numeric sort key | low |

---

## Implementation Order (recommended)

```text
Session 1:  Phase 1 partial — trig (asin/acos/atan/atan2), pmin/pmax
Session 2:  Phase 6 partial — getwd, setwd, list.files, dir.exists, file ops
Session 3:  Phase 5         — add regex crate, upgrade grep/sub/gsub
Session 4:  Phase 9         — runif, rnorm, sample, set.seed
Session 5:  Phase 2 partial — strrep, formatC, utf8 conversion
Session 6:  Phase 7 partial — proc.time (real), system.time (real)
Session 7:  Phase 8         — system(), system2(), Sys.setenv
Session 8:  Phase 3 partial — factor(), table(), rank()
Session 9:  Phase 4         — ls, rm, environment, new.env, globalenv
Session 10: Phase 11 partial — read.csv, write.csv
Session 11: Phase 12 partial — apply, mapply, tapply (needs matrix)
```

---

## Functions NOT worth implementing (intentional divergence)

Per CLAUDE.md: "We will diverge from R behavior when R behavior is absurd."

| R function | Why skip |
|---|---|
| S4 classes (`setClass`, `setMethod`, `setGeneric`) | Overcomplicated OOP; S3 + traits suffice |
| `attach()` / `detach()` | Pollutes global namespace, widely considered harmful |
| `<<-` deep assignment | Already works but discourage; explicit env assignment better |
| `.Internal()` / `.Primitive()` / `.Call()` | GNU R internals, not applicable |
| `tracemem()` / `retracemem()` | GNU R memory debugging |
| `reg.finalizer()` | GC finalizers — Rust handles memory differently |
| `Encoding()` / `enc2utf8()` | miniR is UTF-8 only |
