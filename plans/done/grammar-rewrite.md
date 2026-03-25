# Grammar Rewrite Plan

Status: **Complete**

## Goal

Rewrite `src/parser/r.pest` PEG grammar to match R Language Definition operator precedence exactly, and update the full pipeline (AST, parser, interpreter) to match.

## Steps

1. [x] Rewrite `r.pest` grammar with correct R precedence (16 levels)
   - `::` `:::` > `$` `@` > `[` `[[` > `^` > unary `+` `-` > `:` > `%any%` `|>` > `*` `/` > `+` `-` > comparisons > `!` > `&` `&&` > `|` `||` > `~` > `->` `>>` > `<-` `<<-` > `=`
   - Keyword boundary checks (`if` not matching `ifx`)
   - Empty arguments: `f(1,,3)`, `f(,)`, `f(x=)`
   - Complex number literals (`1i`, `2.5i`)
   - Hex floats (`0x1.fp3`)
   - `..1`, `..2` dotdot tokens
   - `**` as power synonym for `^`

2. [x] Update AST types (`src/parser/ast.rs`)
   - Added `Namespace`, `DotDot`, `Complex`, `Formula` at tilde level
   - 28 Expr variants total

3. [x] Rewrite `src/parser/mod.rs` to convert new pest pairs to AST

4. [x] Update interpreter (`src/interpreter/mod.rs`) for new AST nodes
   - Namespace, DotDot, Complex, Formula (stubs where needed)

5. [x] Create base environment (builtins) as parent of global environment
   - Implements R's findFun: symbol in call position skips non-function bindings

6. [x] Build and fix compilation errors

7. [x] Test with R snippets
   - `tests/grammar_test.R` — basic grammar (32 outputs)
   - `tests/grammar_advanced.R` — complex/hex/repeat/next/return (25 outputs)
   - `tests/grammar_edge.R` — backticks, chained indexing, pipes, `sum(1:100)` (19 outputs)

## Key Decisions Made

- `T` and `F` are reassignable identifiers (not keywords), bound to `TRUE`/`FALSE` in base env
- `**` is a power operator synonym for `^`
- Base env holds builtins; global env is a child — mirrors real R's environment chain
- `get_function()` in Environment does findFun-style lookup (skips non-function bindings)
