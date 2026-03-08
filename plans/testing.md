# testing dependencies integration plan

> Dev/bench dependencies for newr's test suite.

## assert_cmd 2.1

CLI integration testing. Run `newr` as a subprocess and assert on stdout/stderr/exit code.

```rust
use assert_cmd::Command;

#[test]
fn test_script_execution() {
    Command::cargo_bin("newr").unwrap()
        .arg("tests/basic.R")
        .assert()
        .success()
        .stdout(predicates::str::contains("[1] 42"));
}
```

**Use for:** End-to-end tests of `newr script.R` and `newr -e "expr"` modes.

## pretty_assertions 1.4

Drop-in replacement for `assert_eq!` with colorful diffs on failure. Shows
exactly which characters differ in long strings.

**Use for:** All tests comparing R output strings — makes failures readable.

## rstest 0.26

Parametrized tests via `#[rstest]` attribute:

```rust
#[rstest]
#[case("1 + 1", "2")]
#[case("paste('a', 'b')", "a b")]
#[case("length(1:10)", "10")]
fn test_eval(#[case] input: &str, #[case] expected: &str) {
    assert_eq!(eval(input), expected);
}
```

Also provides `#[fixture]` for test setup.

**Use for:** Parametrized R expression evaluation tests — one test function,
many input/output pairs.

## serial_test 3.4

`#[serial]` attribute to run tests sequentially (not in parallel):

```rust
#[test]
#[serial]
fn test_global_env_mutation() {
    // Tests that modify global state must not run concurrently
}
```

**Use for:** Tests that modify shared interpreter state or global environment.

## fancy-regex 0.17

Regex with backreferences and lookaround (not supported by `regex` crate).

```rust
let re = fancy_regex::Regex::new(r"(\w+)\s+\1")?;  // backreference
let re = fancy_regex::Regex::new(r"(?<=@)\w+")?;    // lookbehind
```

**Use for:** R's `grep()`, `sub()`, `gsub()` with Perl-compatible regex features.
Not just a dev dependency — this should be a runtime dependency for PCRE-style regex.

## tempfile 3.26

Create temporary files and directories that are automatically cleaned up:

```rust
let dir = tempfile::tempdir()?;
let file = tempfile::NamedTempFile::new()?;
```

**Use for:** Tests that create files (`write.csv`, `save`, `source`),
and R's `tempfile()` / `tempdir()` builtins.

## divan 0.1

Benchmarking framework with statistical analysis:

```rust
#[divan::bench]
fn bench_format_double() -> String {
    format_r_double(std::f64::consts::PI)
}
```

**Use for:** Benchmarking hot paths — `format_r_double()`, vector operations,
parsing, evaluation.

## quickcheck (BurntSushi)

Property-based testing — generates random inputs and checks invariants:

```rust
#[quickcheck]
fn double_roundtrip(x: f64) -> bool {
    let s = format_r_double(x);
    s.parse::<f64>().unwrap() == x || x.is_nan()
}
```

**Use for:** Testing R builtins with random inputs — coercion roundtrips,
arithmetic properties, string operations.

## Recommendation

**Add incrementally:**
1. `pretty_assertions` + `rstest` — add now, improve test readability immediately
2. `assert_cmd` — add when testing CLI modes
3. `serial_test` — add when tests need sequential execution
4. `tempfile` — add when testing file I/O builtins
5. `fancy-regex` — move to runtime dependency for R regex support
6. `divan` — add when benchmarking
7. `quickcheck` — add when property-testing builtins
