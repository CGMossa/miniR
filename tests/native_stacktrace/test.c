/*
 * Test C code for native stacktrace verification.
 * Three nested C functions — the deepest one calls Rf_error.
 *
 * Compile:
 *   cc -c -fPIC -g -fno-omit-frame-pointer \
 *      -I include -I include/miniR -o /tmp/stacktest.o tests/native_stacktrace/test.c
 *   cc -shared -g -o /tmp/stacktest.dylib /tmp/stacktest.o -undefined dynamic_lookup
 *   dsymutil /tmp/stacktest.dylib  # macOS: generate .dSYM for file:line info
 *
 * Run:
 *   MINIR_INCLUDE=include ./target/debug/r -e '
 *     dyn.load("/tmp/stacktest.dylib")
 *     validate <- function(x) .Call("C_validate", as.integer(x))
 *     run_check <- function(x) validate(x)
 *     run_check(-5)
 *   '
 *
 * Expected output:
 *   Error: value must be non-negative, got -5
 *   Traceback (most recent call last):
 *   2: validate(x)
 *      [C] deep_helper at test.c:30 (stacktest.dylib)
 *      [C] middle_helper at test.c:36 (stacktest.dylib)
 *      [C] C_validate at test.c:41 (stacktest.dylib)
 *   1: run_check(-5)
 */

#include <Rinternals.h>

/* Third function — this is where the error happens */
static void deep_helper(int x) {
    if (x < 0) {
        Rf_error("value must be non-negative, got %d", x);
    }
}

/* Second function — calls deep_helper */
static void middle_helper(SEXP val) {
    int x = INTEGER(val)[0];
    deep_helper(x);
}

/* Entry point from R — calls middle_helper */
SEXP C_validate(SEXP x) {
    middle_helper(x);
    return x;
}
