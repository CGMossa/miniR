# GNU R Binary Serialization Plan

Based on `doc/manual/R-ints.texi` section "Serialization Formats".

## Format Overview

R uses version 2/3 binary serialization for `readRDS`/`saveRDS`/`load`/`save`.

### File structure

**saveRDS**: raw serialized object (no header line)
**save**: header line (`RDX2\n` for binary, `RDA2\n` for ASCII) + serialized pairlist

### Serialization stream

1. Format byte: `X\n` (XDR binary), `A\n` (ASCII), `B\n` (native binary)
2. Three integers: format version, R version that wrote, minimum R version to read
3. Recursive object data via `WriteItem`

### Object encoding

Each object starts with a flags integer:
- Bits 0:7 — SEXPTYPE (0:25 used, 241:255 for pseudo-SEXPTYPEs)
- Bit 8 — object bit (has class)
- Bit 9 — has attributes
- Bit 10 — has tag
- Bits 12:27 — gp field

### SEXPTYPEs we need to handle

| Code | SEXPTYPE | R type | miniR mapping |
|------|----------|--------|---------------|
| 0 | NILSXP | NULL | RValue::Null |
| 1 | SYMSXP | symbol | Expr::Symbol (or string) |
| 2 | LISTSXP | pairlist | RList (internal) |
| 4 | CLOSXP | closure | RFunction::Closure |
| 6 | LANGSXP | language | RValue::Language |
| 9 | CHARSXP | internal string | String |
| 10 | LGLSXP | logical | Vector::Logical |
| 13 | INTSXP | integer | Vector::Integer |
| 14 | REALSXP | double | Vector::Double |
| 15 | CPLXSXP | complex | Vector::Complex |
| 16 | STRSXP | character | Vector::Character |
| 19 | VECSXP | list | RList |
| 20 | EXPRSXP | expression | list of Language |
| 24 | RAWSXP | raw | Vector::Raw |
| 25 | OBJSXP | S4 object | list with class |

### Pseudo-SEXPTYPEs (special singletons)

| Code | Meaning |
|------|---------|
| 242 | R_EmptyEnv |
| 243 | R_BaseEnv |
| 244 | R_GlobalEnv |
| 245 | R_UnboundValue |
| 246 | R_MissingArg |
| 247 | R_BaseNamespace |
| 254 | R_NilValue |
| 255 | reference to previously seen object |

### XDR binary format

- Integers: big-endian 32-bit
- Doubles: big-endian 64-bit (IEEE 754)
- Strings: length (int) + raw bytes (NOT padded to 4 bytes)
- NA_STRING: length = -1, no data
- Long vectors (>2^31-1): length = -1, then two 32-bit ints for upper/lower

### Attributes

Stored as a pairlist after the object data (when has-attributes bit is set).
Each attribute is a TAG (symbol) + value pair.

### Reference objects

Environments, external pointers, and weak references are tracked in a hash
table during serialization. First occurrence writes the full object;
subsequent occurrences write a reference index (pseudo-SEXPTYPE 255).

### Version 3 additions

- ALTREP support (custom serialization for alternative representations)
- Native encoding stored at serialization time
- We can start with version 2 and add v3 later

## Implementation Plan

### Phase 1: Read version 2 XDR binary

Create `src/interpreter/builtins/serialize.rs`:

1. **`unserialize_xdr(bytes: &[u8]) -> Result<RValue, RError>`**
   - Read format header (X/A/B + version + R versions)
   - Implement `read_item()` recursive deserializer
   - Handle: NILSXP, LGLSXP, INTSXP, REALSXP, STRSXP, VECSXP, RAWSXP, CPLXSXP
   - Handle: CHARSXP (internal strings with encoding bits)
   - Handle: LISTSXP pairlists → convert to RList
   - Handle: attributes as pairlists → convert to Attributes
   - Handle: pseudo-SEXPTYPEs (NULL, EmptyEnv, GlobalEnv, etc.)
   - Handle: reference objects via index table

2. **`readRDS(file)` upgrade** — detect binary format, call unserialize_xdr

3. **`load(file)` upgrade** — detect RDX2 header, deserialize pairlist, assign

### Phase 2: Write version 2 XDR binary

1. **`serialize_xdr(value: &RValue) -> Vec<u8>`**
   - Write format header
   - Implement `write_item()` recursive serializer
   - Track reference objects in hash table
   - Handle all SEXPTYPEs from Phase 1

2. **`saveRDS(object, file)` upgrade** — serialize to XDR binary

3. **`save(..., file)` upgrade** — write RDX2 header + serialized pairlist

### Phase 3: Compression

- Read: detect gzip/bzip2/xz headers, decompress before deserializing
- Write: gzip compression by default (use `flate2` crate)
- `flate2` is a common Rust crate for gzip; add as optional dep

### Phase 4: ASCII format

- Lower priority — binary is the common case
- Implement `A\n` format for `save(ascii=TRUE)`

## Key challenges

1. **Environment serialization**: Need to serialize/deserialize environment
   chains. Package/namespace envs write their name; normal envs write their
   full contents.

2. **Closure serialization**: Closures have formals (pairlist), body (LANGSXP),
   and environment. Need full language object serialization.

3. **Reference tracking**: Hash table for shared objects. Must preserve
   object identity through serialization round-trips.

4. **Endianness**: XDR is big-endian. Use `u32::to_be_bytes()` / `from_be_bytes()`.

## Dependencies

- Phase 1-2: `flate2` crate for gzip decompression (nearly all .rds files are gzip-compressed)
- Phase 3: Same `flate2` crate for gzip write
- See `plans/flate2.md` for flate2 integration details
- Use `flate2 = { version = "1", features = ["rust_backend"] }` for pure-Rust (no system zlib)

## First deliverable

`readRDS("file.rds")` reads a binary RDS file written by GNU R containing
a numeric vector, character vector, or data.frame. This unblocks reading
real R data files.

## Priority: HIGH — this is the #2 blocker after package loading.
