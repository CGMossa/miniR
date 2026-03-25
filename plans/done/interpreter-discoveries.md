# Interpreter discoveries from thread-local refactor

Issues and improvements found while reading through the full interpreter codebase.

## Builtins that don't need interpreter access

These are registered as `#[interpreter_builtin]` but never call `with_interpreter()`. They could be plain `#[builtin]` functions with an added `&Environment` parameter, or just regular builtins:

- `interp_vectorize` — returns first arg, no interpreter or env needed
- `interp_switch` — pure value dispatch, no interpreter needed
- `interp_get` — only uses `env.get()`, could be a regular builtin with env
- `interp_assign` — only uses `env.set()`, same
- `interp_exists` — only uses `env.get()`, same
- `interp_system_time` — just measures wall time, no interpreter or env needed

Converting these to `#[builtin]` would skip the interpreter-builtin dispatch check in `call_function`, which is a (small) performance win on every builtin call.

## `Reduce` doesn't do `match.fun`

`Reduce("+", 1:10)` fails because our Reduce passes the string `"+"` to `call_function`, which expects an `RValue::Function`. R's `Reduce` calls `match.fun(f)` first to convert strings and symbols to function values. Need a `match_fun` helper that resolves strings to functions via env lookup.

## `eval_apply` uses global env instead of calling env

In `eval_apply` (used by sapply/lapply/vapply), the env is hardcoded to `interp.global_env`. It should use the env from the call site. Before the refactor it was the same bug (`interp.global_env.clone()`). The correct fix: pass the `env: &Environment` parameter through from the builtin's `_env` argument.

## `InterpreterBuiltinFn` and `BuiltinFn` are nearly identical

After the refactor:

- `BuiltinFn = fn(&[RValue], &[(String, RValue)]) -> Result<RValue, RError>`
- `InterpreterBuiltinFn = fn(&[RValue], &[(String, RValue)], &Environment) -> Result<RValue, RError>`

The only difference is `&Environment`. Consider merging into a single type that always receives `&Environment`. This would eliminate the two-registry dispatch pattern and simplify `call_function`.

## `%in%` operator converts everything to character

`eval_in_op` in interpreter.rs converts both sides to `to_characters()` before comparison. This means `1 %in% c(1, 2, 3)` works by string comparison (`"1" in ["1", "2", "3"]`) rather than numeric comparison. This is accidentally correct for simple cases but wrong for `1.0 %in% 1L` or floating point edge cases.

## `is_assignment_or_invisible` is crude string matching

In main.rs, this function checks `trimmed.contains("=")` to detect assignments, which gives false positives for things like `grepl("=", x)` or `paste("a=b")`. A proper solution would check the parsed AST for assignment nodes rather than doing string heuristics.

## `eval_dollar` treats `@` same as `$`

Line 100 in interpreter.rs: `Expr::Slot { object, member } => self.eval_dollar(object, member, env)` with comment "treat like $". R's `@` is for S4 slot access and should check for S4 objects / validate slot names. Low priority since S4 is complex.

## Missing `format_r_double` locality

`format_r_double` is used in builtins.rs (cat, str) but defined somewhere in value.rs. It handles R-style float formatting. Worth checking it covers all R edge cases (Inf, -Inf, NaN, integer-valued doubles printing without decimal).
