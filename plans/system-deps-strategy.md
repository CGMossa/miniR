# System Dependencies Strategy

## Status (2026-04-03)

pkg-config integration implemented in compile.rs. Packages with `Makevars.in`
but no `Makevars` now have their `@cflags@`/`@libs@` placeholders resolved
via pkg-config, replicating R's "anticonf" configure pattern.

## Approach

**pkg-config is the primary mechanism.** When a CRAN package has `Makevars.in`
but no `Makevars` (i.e. its configure script hasn't been run), we:

1. Detect the pkg-config library name from a hardcoded map or the `configure`
   script's `PKG_CONFIG_NAME="..."` variable
2. Run `pkg-config --cflags --libs <name>` via the `pkg-config` Rust crate
3. Substitute `@cflags@` and `@libs@` placeholders in `Makevars.in`
4. Strip remaining `@VAR@` placeholders to prevent compiler errors

Users must install system libraries (e.g. `brew install openssl libxml2 icu4c`).

## Per-package status

| Package | System dep | pkg-config name | Status |
|---------|-----------|----------------|--------|
| xml2 | libxml2 | `libxml-2.0` | **Works** |
| openssl | OpenSSL | `openssl` | Compiles, runtime segfault (C API gap) |
| curl | libcurl | `libcurl` | Already works via Connections.h fix |
| stringi | ICU | `icu-i18n` | Needs full configure emulation (custom vars) |
| fs | libuv | `libuv` | Bundles libuv source, needs autotools-like build |
| Matrix | SuiteSparse | N/A | Bundles SuiteSparse, needs `SuiteSparse_config.h` |
| ps | platform APIs | N/A | Needs generated `config.h` |
| sodium | libsodium | `libsodium` | Not yet tested |

## Known pkg-config names

Hardcoded in `compile.rs::pkg_config_name_for_package()`:

- openssl → `openssl`
- xml2 → `libxml-2.0`
- stringi → `icu-i18n`
- curl → `libcurl`
- sodium → `libsodium`
- cairo → `cairo`
- RPostgres → `libpq`
- magick → `Magick++`
- poppler → `poppler-cpp`
- pdftools → `poppler-glib`
- rsvg → `librsvg-2.0`

## Next steps

- Fix openssl runtime segfault (missing C API in dyn.load init)
- Generate `config.h` for `ps` package from platform detection
- For stringi: provide a pre-configured Makevars for macOS/Linux
  (stringi's configure is too complex to replicate — it selects source
  files and ICU bundling strategy)
- For Matrix: vendor the `SuiteSparse_config.h` header
- For fs: investigate if system libuv can be used instead of bundled
