# serde_json integration plan

> `serde_json` 1.0 — JSON serialization/deserialization.
> <https://github.com/serde-rs/json>

## What it does

Parse JSON into `serde_json::Value` (untyped) or into typed Rust structs via serde.
Serialize Rust values back to JSON strings.

```rust
let v: Value = serde_json::from_str(r#"{"name": "Alice", "age": 30}"#)?;
let s = serde_json::to_string_pretty(&v)?;
```

`Value` enum: `Null`, `Bool(bool)`, `Number(Number)`, `String(String)`,
`Array(Vec<Value>)`, `Object(Map<String, Value>)`.

## Where it fits in newr

### 1. `jsonlite::fromJSON()` / `jsonlite::toJSON()`

R's `jsonlite` package is the standard for JSON. We can provide built-in JSON support:

```r
x <- fromJSON('{"a": [1,2,3], "b": "hello"}')
# x is a named list: list(a = c(1,2,3), b = "hello")

toJSON(list(x = 1:3, y = "abc"))
# '{"x":[1,2,3],"y":"abc"}'
```

Mapping:

- JSON `null` → R `NULL`
- JSON `bool` → R `logical`
- JSON `number` → R `double` (or `integer` if whole)
- JSON `string` → R `character`
- JSON `array` → R `vector` (if homogeneous) or `list`
- JSON `object` → R named `list`

### 2. Configuration files

Read newr config files (`.newr.json`, package metadata) in JSON format.

### 3. `httr` / HTTP response parsing

When we implement HTTP builtins, JSON is the primary response format.

## Relationship to builtins plan

| Phase | Builtins affected | Impact |
|---|---|---|
| Phase 2 (strings) | `fromJSON()`, `toJSON()` | JSON ↔ R conversion |
| Phase 11 (I/O) | config file reading | JSON config support |

## Recommendation

**Add when implementing JSON builtins.** serde_json is the de facto standard,
zero question about the choice. The R↔JSON mapping is well-defined.

**Effort:** 1-2 hours for basic fromJSON/toJSON.
