/*
 * native_trampoline.c — compiled into the miniR binary via build.rs.
 *
 * Contains ONLY the setjmp/longjmp pair for Rf_error protection.
 * Everything else is implemented in Rust (src/interpreter/native/runtime.rs).
 *
 * The call stack during .Call:
 *   Rust: dot_call()
 *     → C: _minir_call_protected()  [setjmp here]
 *       → C (package .so): native_function()
 *         → C: Rf_error()  [longjmp here — only crosses C frames]
 *     → C: returns error code to Rust
 *   Rust: reads error code + message
 */

#include <setjmp.h>
#include <stdio.h>
#include <stdarg.h>
#include <string.h>
#include <stdint.h>

/* Native backtrace capture for stacktrace support.
 * Available on macOS (always), Linux with glibc, and Windows (MSVC). */
#if defined(__APPLE__) || defined(__GLIBC__)
#include <execinfo.h>
#define HAVE_BACKTRACE 1
#elif defined(_WIN32)
#include <windows.h>
#define HAVE_BACKTRACE 1
#else
#define HAVE_BACKTRACE 0
#endif

/* Forward-declare SEXP for function signatures */
struct SEXPREC;
typedef struct SEXPREC *SEXP;

/* ── Shared state for the setjmp/longjmp pair ── */

static jmp_buf _error_jmp;
static char    _error_msg[4096];
static int     _has_error = 0;

/* Native backtrace captured in Rf_error() before longjmp. */
#define MAX_BT_FRAMES 64
static void *_bt_frames[MAX_BT_FRAMES];
static int   _bt_count = 0;

/* Platform-abstracted backtrace capture. */
static int capture_backtrace(void **frames, int max_frames) {
#if defined(_WIN32)
    return (int)CaptureStackBackTrace(0, (DWORD)max_frames, frames, NULL);
#elif HAVE_BACKTRACE
    return backtrace(frames, max_frames);
#else
    (void)frames; (void)max_frames;
    return 0;
#endif
}

/* ── Rf_error / Rf_warning — called by package C code ── */

/* These are declared in Rinternals.h and resolved from the binary */

void Rf_error(const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(_error_msg, sizeof(_error_msg), fmt, ap);
    va_end(ap);
    _has_error = 1;
    _bt_count = capture_backtrace(_bt_frames, MAX_BT_FRAMES);
    longjmp(_error_jmp, 1);
}

void Rf_errorcall(SEXP call, const char *fmt, ...) {
    (void)call;
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(_error_msg, sizeof(_error_msg), fmt, ap);
    va_end(ap);
    _has_error = 1;
    _bt_count = capture_backtrace(_bt_frames, MAX_BT_FRAMES);
    longjmp(_error_jmp, 1);
}

void Rf_warning(const char *fmt, ...) {
    va_list ap;
    char buf[4096];
    va_start(ap, fmt);
    vsnprintf(buf, sizeof(buf), fmt, ap);
    va_end(ap);
    fprintf(stderr, "Warning: %s\n", buf);
}

void Rf_warningcall(SEXP call, const char *fmt, ...) {
    (void)call;
    Rf_warning(fmt);
}

/* ── Printing (must be C for va_list) ── */

void Rprintf(const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    vfprintf(stdout, fmt, ap);
    va_end(ap);
}

void REprintf(const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    vfprintf(stderr, fmt, ap);
    va_end(ap);
}

void Rvprintf(const char *fmt, va_list ap) {
    vfprintf(stdout, fmt, ap);
}

void REvprintf(const char *fmt, va_list ap) {
    vfprintf(stderr, fmt, ap);
}

/* ── Protected call trampoline ── */

typedef SEXP (*_minir_dotcall_fn)();

int _minir_call_protected(_minir_dotcall_fn fn, SEXP *args, int nargs, SEXP *result) {
    _has_error = 0;
    _error_msg[0] = '\0';
    _bt_count = 0;

    if (setjmp(_error_jmp) != 0) {
        *result = (SEXP)0;
        return 1;
    }

    typedef SEXP (*F0)(void);
    typedef SEXP (*F1)(SEXP);
    typedef SEXP (*F2)(SEXP,SEXP);
    typedef SEXP (*F3)(SEXP,SEXP,SEXP);
    typedef SEXP (*F4)(SEXP,SEXP,SEXP,SEXP);
    typedef SEXP (*F5)(SEXP,SEXP,SEXP,SEXP,SEXP);
    typedef SEXP (*F6)(SEXP,SEXP,SEXP,SEXP,SEXP,SEXP);
    typedef SEXP (*F7)(SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP);
    typedef SEXP (*F8)(SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP);
    typedef SEXP (*F9)(SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP);
    typedef SEXP (*F10)(SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP);
    typedef SEXP (*F11)(SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP);
    typedef SEXP (*F12)(SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP);
    typedef SEXP (*F13)(SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP);
    typedef SEXP (*F14)(SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP);
    typedef SEXP (*F15)(SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP);
    typedef SEXP (*F16)(SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP,SEXP);

    #define A(i) args[i]
    switch (nargs) {
        case 0:  *result = ((F0)fn)(); break;
        case 1:  *result = ((F1)fn)(A(0)); break;
        case 2:  *result = ((F2)fn)(A(0),A(1)); break;
        case 3:  *result = ((F3)fn)(A(0),A(1),A(2)); break;
        case 4:  *result = ((F4)fn)(A(0),A(1),A(2),A(3)); break;
        case 5:  *result = ((F5)fn)(A(0),A(1),A(2),A(3),A(4)); break;
        case 6:  *result = ((F6)fn)(A(0),A(1),A(2),A(3),A(4),A(5)); break;
        case 7:  *result = ((F7)fn)(A(0),A(1),A(2),A(3),A(4),A(5),A(6)); break;
        case 8:  *result = ((F8)fn)(A(0),A(1),A(2),A(3),A(4),A(5),A(6),A(7)); break;
        case 9:  *result = ((F9)fn)(A(0),A(1),A(2),A(3),A(4),A(5),A(6),A(7),A(8)); break;
        case 10: *result = ((F10)fn)(A(0),A(1),A(2),A(3),A(4),A(5),A(6),A(7),A(8),A(9)); break;
        case 11: *result = ((F11)fn)(A(0),A(1),A(2),A(3),A(4),A(5),A(6),A(7),A(8),A(9),A(10)); break;
        case 12: *result = ((F12)fn)(A(0),A(1),A(2),A(3),A(4),A(5),A(6),A(7),A(8),A(9),A(10),A(11)); break;
        case 13: *result = ((F13)fn)(A(0),A(1),A(2),A(3),A(4),A(5),A(6),A(7),A(8),A(9),A(10),A(11),A(12)); break;
        case 14: *result = ((F14)fn)(A(0),A(1),A(2),A(3),A(4),A(5),A(6),A(7),A(8),A(9),A(10),A(11),A(12),A(13)); break;
        case 15: *result = ((F15)fn)(A(0),A(1),A(2),A(3),A(4),A(5),A(6),A(7),A(8),A(9),A(10),A(11),A(12),A(13),A(14)); break;
        case 16: *result = ((F16)fn)(A(0),A(1),A(2),A(3),A(4),A(5),A(6),A(7),A(8),A(9),A(10),A(11),A(12),A(13),A(14),A(15)); break;
        default:
            snprintf(_error_msg, sizeof(_error_msg), ".Call with %d arguments not supported (max 16)", nargs);
            _has_error = 1;
            *result = (SEXP)0;
            return 1;
    }
    #undef A
    return 0;
}

/* ── Protected .C call trampoline ── */
/* .C functions take void* pointers and return void. */

typedef void (*_minir_dotC_fn)();

int _minir_dotC_call_protected(_minir_dotC_fn fn, void **args, int nargs) {
    _has_error = 0;
    _error_msg[0] = '\0';
    _bt_count = 0;

    if (setjmp(_error_jmp) != 0) {
        return 1;
    }

    typedef void (*C0)(void);
    typedef void (*C1)(void*);
    typedef void (*C2)(void*,void*);
    typedef void (*C3)(void*,void*,void*);
    typedef void (*C4)(void*,void*,void*,void*);
    typedef void (*C5)(void*,void*,void*,void*,void*);
    typedef void (*C6)(void*,void*,void*,void*,void*,void*);
    typedef void (*C7)(void*,void*,void*,void*,void*,void*,void*);
    typedef void (*C8)(void*,void*,void*,void*,void*,void*,void*,void*);
    typedef void (*C9)(void*,void*,void*,void*,void*,void*,void*,void*,void*);
    typedef void (*C10)(void*,void*,void*,void*,void*,void*,void*,void*,void*,void*);
    typedef void (*C11)(void*,void*,void*,void*,void*,void*,void*,void*,void*,void*,void*);
    typedef void (*C12)(void*,void*,void*,void*,void*,void*,void*,void*,void*,void*,void*,void*);
    typedef void (*C13)(void*,void*,void*,void*,void*,void*,void*,void*,void*,void*,void*,void*,void*);
    typedef void (*C14)(void*,void*,void*,void*,void*,void*,void*,void*,void*,void*,void*,void*,void*,void*);
    typedef void (*C15)(void*,void*,void*,void*,void*,void*,void*,void*,void*,void*,void*,void*,void*,void*,void*);
    typedef void (*C16)(void*,void*,void*,void*,void*,void*,void*,void*,void*,void*,void*,void*,void*,void*,void*,void*);

    #define B(i) args[i]
    switch (nargs) {
        case 0:  ((C0)fn)(); break;
        case 1:  ((C1)fn)(B(0)); break;
        case 2:  ((C2)fn)(B(0),B(1)); break;
        case 3:  ((C3)fn)(B(0),B(1),B(2)); break;
        case 4:  ((C4)fn)(B(0),B(1),B(2),B(3)); break;
        case 5:  ((C5)fn)(B(0),B(1),B(2),B(3),B(4)); break;
        case 6:  ((C6)fn)(B(0),B(1),B(2),B(3),B(4),B(5)); break;
        case 7:  ((C7)fn)(B(0),B(1),B(2),B(3),B(4),B(5),B(6)); break;
        case 8:  ((C8)fn)(B(0),B(1),B(2),B(3),B(4),B(5),B(6),B(7)); break;
        case 9:  ((C9)fn)(B(0),B(1),B(2),B(3),B(4),B(5),B(6),B(7),B(8)); break;
        case 10: ((C10)fn)(B(0),B(1),B(2),B(3),B(4),B(5),B(6),B(7),B(8),B(9)); break;
        case 11: ((C11)fn)(B(0),B(1),B(2),B(3),B(4),B(5),B(6),B(7),B(8),B(9),B(10)); break;
        case 12: ((C12)fn)(B(0),B(1),B(2),B(3),B(4),B(5),B(6),B(7),B(8),B(9),B(10),B(11)); break;
        case 13: ((C13)fn)(B(0),B(1),B(2),B(3),B(4),B(5),B(6),B(7),B(8),B(9),B(10),B(11),B(12)); break;
        case 14: ((C14)fn)(B(0),B(1),B(2),B(3),B(4),B(5),B(6),B(7),B(8),B(9),B(10),B(11),B(12),B(13)); break;
        case 15: ((C15)fn)(B(0),B(1),B(2),B(3),B(4),B(5),B(6),B(7),B(8),B(9),B(10),B(11),B(12),B(13),B(14)); break;
        case 16: ((C16)fn)(B(0),B(1),B(2),B(3),B(4),B(5),B(6),B(7),B(8),B(9),B(10),B(11),B(12),B(13),B(14),B(15)); break;
        default:
            snprintf(_error_msg, sizeof(_error_msg), ".C with %d arguments not supported (max 16)", nargs);
            _has_error = 1;
            return 1;
    }
    #undef B
    return 0;
}

const char *_minir_get_error_msg(void) { return _error_msg; }
int _minir_has_error_flag(void) { return _has_error; }
int _minir_bt_count(void) { return _bt_count; }
void *const *_minir_bt_frames(void) { return _bt_frames; }
