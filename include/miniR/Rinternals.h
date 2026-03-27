/*
 * miniR — Rinternals.h
 *
 * C-compatible header for R packages compiled against miniR.
 * Defines the SEXP type, accessor macros, and API function declarations.
 *
 * This is miniR's own ABI — simpler than GNU R's SEXPREC but compatible
 * with the public C API that most R packages use.
 */

#ifndef MINIR_RINTERNALS_H
#define MINIR_RINTERNALS_H

#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <stdarg.h>
#include <setjmp.h>
#include <math.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ── SEXPTYPE constants ── */

typedef unsigned int SEXPTYPE;

#define NILSXP      0
#define SYMSXP      1
#define LISTSXP     2
#define CLOSXP      3
#define ENVSXP      4
#define PROMSXP     5
#define LANGSXP     6
#define SPECIALSXP  7
#define BUILTINSXP  8
#define CHARSXP     9
#define LGLSXP      10
#define INTSXP      13
#define REALSXP     14
#define CPLXSXP     15
#define STRSXP      16
#define DOTSXP      17
#define ANYSXP      18
#define VECSXP      19
#define EXPRSXP     20
#define BCODESXP    21
#define EXTPTRSXP   22
#define WEAKREFSXP  23
#define RAWSXP      24
#define OBJSXP      25

/* ── Rboolean ── */

typedef enum { FALSE = 0, TRUE = 1 } Rboolean;

/* ── Basic types ── */

typedef int R_len_t;
typedef ptrdiff_t R_xlen_t;
typedef unsigned char Rbyte;

typedef struct { double r; double i; } Rcomplex;

/* ── SEXPREC structure ── */

struct SEXPREC {
    uint8_t  type;      /* SEXPTYPE tag */
    uint8_t  flags;     /* GC mark, named count (currently unused) */
    uint16_t padding;
    int32_t  length;    /* vector length */
    void    *data;      /* pointer to data buffer (type-dependent) */
    struct SEXPREC *attrib; /* attributes (NULL if none) */
};

typedef struct SEXPREC *SEXP;

/* ── NA values ── */

/* NA_REAL: R's canonical NA for doubles — a specific NaN with payload 1954 */
static inline double R_NaReal(void) {
    union { uint64_t u; double d; } na;
    na.u = 0x7FF00000000007A2ULL;
    return na.d;
}
#define NA_REAL     R_NaReal()
#define NA_INTEGER  (-2147483647 - 1)  /* INT32_MIN */
#define NA_LOGICAL  NA_INTEGER

#define R_NaInt     NA_INTEGER

/* Check if a double is R's NA (not just any NaN) */
static inline int R_IsNA(double x) {
    union { double d; uint64_t u; } val;
    val.d = x;
    return val.u == 0x7FF00000000007A2ULL;
}

#define ISNA(x)  R_IsNA(x)
#define ISNAN(x) (isnan(x))

/* ── R_NilValue ── */

/* Statically allocated NILSXP sentinel */
static struct SEXPREC _R_NilValue_rec = { NILSXP, 0, 0, 0, NULL, NULL };
#define R_NilValue  (&_R_NilValue_rec)

/* ── Accessor macros ── */

#define TYPEOF(x)    ((SEXPTYPE)((x)->type))
#define LENGTH(x)    ((x)->length)
#define XLENGTH(x)   ((R_xlen_t)((x)->length))

#define REAL(x)      ((double*)((x)->data))
#define INTEGER(x)   ((int*)((x)->data))
#define LOGICAL(x)   ((int*)((x)->data))
#define RAW(x)       ((Rbyte*)((x)->data))
#define COMPLEX(x)   ((Rcomplex*)((x)->data))

/* String element access */
#define STRING_ELT(x, i)   (((SEXP*)((x)->data))[i])
#define VECTOR_ELT(x, i)   (((SEXP*)((x)->data))[i])
#define SET_STRING_ELT(x, i, v)  (((SEXP*)((x)->data))[i] = (v))
#define SET_VECTOR_ELT(x, i, v)  (((SEXP*)((x)->data))[i] = (v))

/* CHARSXP data access */
#define R_CHAR(x)    ((const char*)((x)->data))
#define CHAR(x)      R_CHAR(x)

/* ── Allocation tracking ── */

#ifndef MINIR_MAX_ALLOCS
#define MINIR_MAX_ALLOCS 65536
#endif

#ifndef MINIR_MAX_PROTECT
#define MINIR_MAX_PROTECT 10000
#endif

static SEXP  _minir_alloc_list[MINIR_MAX_ALLOCS];
static int   _minir_alloc_count = 0;
static SEXP  _minir_protect_stack[MINIR_MAX_PROTECT];
static int   _minir_protect_count = 0;

/* Error handling via setjmp/longjmp */
static jmp_buf _minir_error_jmp;
static char    _minir_error_msg[2048];
static int     _minir_has_error = 0;

static inline void _minir_track_alloc(SEXP s) {
    if (_minir_alloc_count < MINIR_MAX_ALLOCS) {
        _minir_alloc_list[_minir_alloc_count++] = s;
    }
}

/* ── Allocation functions ── */

static inline SEXP Rf_allocVector(SEXPTYPE type, R_xlen_t length) {
    SEXP s = (SEXP)calloc(1, sizeof(struct SEXPREC));
    if (!s) return R_NilValue;
    s->type = (uint8_t)type;
    s->length = (int32_t)length;
    s->attrib = R_NilValue;

    if (length > 0) {
        switch (type) {
            case REALSXP:
                s->data = calloc((size_t)length, sizeof(double));
                break;
            case INTSXP:
            case LGLSXP:
                s->data = calloc((size_t)length, sizeof(int));
                break;
            case STRSXP:
            case VECSXP:
            case EXPRSXP:
                s->data = calloc((size_t)length, sizeof(SEXP));
                break;
            case RAWSXP:
                s->data = calloc((size_t)length, sizeof(Rbyte));
                break;
            case CPLXSXP:
                s->data = calloc((size_t)length, sizeof(Rcomplex));
                break;
            default:
                s->data = NULL;
                break;
        }
    }

    _minir_track_alloc(s);
    return s;
}

#define Rf_allocVector3 Rf_allocVector
#define allocVector     Rf_allocVector

static inline SEXP Rf_allocMatrix(SEXPTYPE type, int nrow, int ncol) {
    return Rf_allocVector(type, (R_xlen_t)nrow * ncol);
}
#define allocMatrix Rf_allocMatrix

/* ── Scalar constructors ── */

static inline SEXP Rf_ScalarReal(double x) {
    SEXP s = Rf_allocVector(REALSXP, 1);
    REAL(s)[0] = x;
    return s;
}
#define ScalarReal Rf_ScalarReal

static inline SEXP Rf_ScalarInteger(int x) {
    SEXP s = Rf_allocVector(INTSXP, 1);
    INTEGER(s)[0] = x;
    return s;
}
#define ScalarInteger Rf_ScalarInteger

static inline SEXP Rf_ScalarLogical(int x) {
    SEXP s = Rf_allocVector(LGLSXP, 1);
    LOGICAL(s)[0] = x;
    return s;
}
#define ScalarLogical Rf_ScalarLogical

/* ── String constructors ── */

static inline SEXP Rf_mkChar(const char *str) {
    size_t len = strlen(str);
    SEXP s = (SEXP)calloc(1, sizeof(struct SEXPREC));
    if (!s) return R_NilValue;
    s->type = CHARSXP;
    s->length = (int32_t)len;
    char *buf = (char*)malloc(len + 1);
    if (buf) {
        memcpy(buf, str, len + 1);
    }
    s->data = buf;
    s->attrib = R_NilValue;
    _minir_track_alloc(s);
    return s;
}
#define mkChar Rf_mkChar

static inline SEXP Rf_mkCharLen(const char *str, int len) {
    SEXP s = (SEXP)calloc(1, sizeof(struct SEXPREC));
    if (!s) return R_NilValue;
    s->type = CHARSXP;
    s->length = (int32_t)len;
    char *buf = (char*)malloc((size_t)len + 1);
    if (buf) {
        memcpy(buf, str, (size_t)len);
        buf[len] = '\0';
    }
    s->data = buf;
    s->attrib = R_NilValue;
    _minir_track_alloc(s);
    return s;
}
#define mkCharLen Rf_mkCharLen

static inline SEXP Rf_mkString(const char *str) {
    SEXP s = Rf_allocVector(STRSXP, 1);
    SET_STRING_ELT(s, 0, Rf_mkChar(str));
    return s;
}
#define mkString Rf_mkString

static inline SEXP Rf_ScalarString(SEXP x) {
    SEXP s = Rf_allocVector(STRSXP, 1);
    SET_STRING_ELT(s, 0, x);
    return s;
}
#define ScalarString Rf_ScalarString

/* ── PROTECT / UNPROTECT ── */

static inline SEXP Rf_protect(SEXP s) {
    if (_minir_protect_count < MINIR_MAX_PROTECT) {
        _minir_protect_stack[_minir_protect_count++] = s;
    }
    return s;
}
#define PROTECT(s) Rf_protect(s)

static inline void Rf_unprotect(int n) {
    _minir_protect_count -= n;
    if (_minir_protect_count < 0) _minir_protect_count = 0;
}
#define UNPROTECT(n) Rf_unprotect(n)

/* R_PreserveObject / R_ReleaseObject — no-ops in miniR */
#define R_PreserveObject(x) ((void)(x))
#define R_ReleaseObject(x)  ((void)(x))

/* ── Type checking ── */

#define Rf_isNull(x)     (TYPEOF(x) == NILSXP)
#define Rf_isReal(x)     (TYPEOF(x) == REALSXP)
#define Rf_isInteger(x)  (TYPEOF(x) == INTSXP)
#define Rf_isLogical(x)  (TYPEOF(x) == LGLSXP)
#define Rf_isString(x)   (TYPEOF(x) == STRSXP)
#define Rf_isVector(x)   (TYPEOF(x) == REALSXP || TYPEOF(x) == INTSXP || TYPEOF(x) == LGLSXP || TYPEOF(x) == STRSXP || TYPEOF(x) == VECSXP || TYPEOF(x) == RAWSXP)

#define isNull     Rf_isNull
#define isReal     Rf_isReal
#define isInteger  Rf_isInteger
#define isLogical  Rf_isLogical
#define isString   Rf_isString
#define isVector   Rf_isVector

/* ── Coercion helpers ── */

static inline double Rf_asReal(SEXP x) {
    switch (TYPEOF(x)) {
        case REALSXP:  return LENGTH(x) > 0 ? REAL(x)[0] : NA_REAL;
        case INTSXP:   return LENGTH(x) > 0 ? (INTEGER(x)[0] == NA_INTEGER ? NA_REAL : (double)INTEGER(x)[0]) : NA_REAL;
        case LGLSXP:   return LENGTH(x) > 0 ? (LOGICAL(x)[0] == NA_LOGICAL ? NA_REAL : (double)LOGICAL(x)[0]) : NA_REAL;
        default:       return NA_REAL;
    }
}
#define asReal Rf_asReal

static inline int Rf_asInteger(SEXP x) {
    switch (TYPEOF(x)) {
        case INTSXP:   return LENGTH(x) > 0 ? INTEGER(x)[0] : NA_INTEGER;
        case REALSXP:  return LENGTH(x) > 0 ? (R_IsNA(REAL(x)[0]) ? NA_INTEGER : (int)REAL(x)[0]) : NA_INTEGER;
        case LGLSXP:   return LENGTH(x) > 0 ? LOGICAL(x)[0] : NA_INTEGER;
        default:       return NA_INTEGER;
    }
}
#define asInteger Rf_asInteger

static inline int Rf_asLogical(SEXP x) {
    switch (TYPEOF(x)) {
        case LGLSXP:   return LENGTH(x) > 0 ? LOGICAL(x)[0] : NA_LOGICAL;
        case INTSXP:   return LENGTH(x) > 0 ? (INTEGER(x)[0] == NA_INTEGER ? NA_LOGICAL : (INTEGER(x)[0] != 0)) : NA_LOGICAL;
        case REALSXP:  return LENGTH(x) > 0 ? (R_IsNA(REAL(x)[0]) ? NA_LOGICAL : (REAL(x)[0] != 0.0)) : NA_LOGICAL;
        default:       return NA_LOGICAL;
    }
}
#define asLogical Rf_asLogical

/* ── Coercion (vector-level) ── */

static inline SEXP Rf_coerceVector(SEXP x, SEXPTYPE type) {
    if (TYPEOF(x) == type) return x;
    R_xlen_t n = XLENGTH(x);
    SEXP out = PROTECT(Rf_allocVector(type, n));
    for (R_xlen_t i = 0; i < n; i++) {
        switch (type) {
            case REALSXP:
                if (TYPEOF(x) == INTSXP)
                    REAL(out)[i] = INTEGER(x)[i] == NA_INTEGER ? NA_REAL : (double)INTEGER(x)[i];
                else if (TYPEOF(x) == LGLSXP)
                    REAL(out)[i] = LOGICAL(x)[i] == NA_LOGICAL ? NA_REAL : (double)LOGICAL(x)[i];
                break;
            case INTSXP:
                if (TYPEOF(x) == REALSXP)
                    INTEGER(out)[i] = R_IsNA(REAL(x)[i]) ? NA_INTEGER : (int)REAL(x)[i];
                else if (TYPEOF(x) == LGLSXP)
                    INTEGER(out)[i] = LOGICAL(x)[i];
                break;
            case LGLSXP:
                if (TYPEOF(x) == INTSXP)
                    LOGICAL(out)[i] = INTEGER(x)[i] == NA_INTEGER ? NA_LOGICAL : (INTEGER(x)[i] != 0);
                else if (TYPEOF(x) == REALSXP)
                    LOGICAL(out)[i] = R_IsNA(REAL(x)[i]) ? NA_LOGICAL : (REAL(x)[i] != 0.0);
                break;
            default:
                break;
        }
    }
    UNPROTECT(1);
    return out;
}
#define coerceVector Rf_coerceVector

/* ── Attributes ── */

static inline SEXP Rf_getAttrib(SEXP x, SEXP name) {
    (void)x; (void)name;
    return R_NilValue;  /* stub — attributes not yet wired */
}
#define getAttrib Rf_getAttrib

static inline SEXP Rf_setAttrib(SEXP x, SEXP name, SEXP val) {
    (void)x; (void)name; (void)val;
    return val;  /* stub */
}
#define setAttrib Rf_setAttrib

/* Common symbol names (stubs — needed to compile but not fully functional) */
#define R_NamesSymbol   R_NilValue
#define R_DimSymbol     R_NilValue
#define R_ClassSymbol   R_NilValue
#define R_RowNamesSymbol R_NilValue
#define R_LevelsSymbol  R_NilValue

/* ── Error handling ── */

static inline void Rf_error(const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(_minir_error_msg, sizeof(_minir_error_msg), fmt, ap);
    va_end(ap);
    _minir_has_error = 1;
    longjmp(_minir_error_jmp, 1);
}
#define error Rf_error

static inline void Rf_warning(const char *fmt, ...) {
    va_list ap;
    char buf[2048];
    va_start(ap, fmt);
    vsnprintf(buf, sizeof(buf), fmt, ap);
    va_end(ap);
    fprintf(stderr, "Warning: %s\n", buf);
}
#define warning Rf_warning

/* Rprintf / REprintf — print to stdout/stderr */
static inline void Rprintf(const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    vfprintf(stdout, fmt, ap);
    va_end(ap);
}

static inline void REprintf(const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    vfprintf(stderr, fmt, ap);
    va_end(ap);
}

/* ── Cleanup (called by miniR after .Call returns) ── */

/* Free a single SEXP's data buffer (not recursively — just the immediate data) */
static inline void _minir_free_sexp_data(SEXP s) {
    if (s && s != R_NilValue && s->data) {
        free(s->data);
        s->data = NULL;
    }
}

/* Free all tracked allocations and reset state.
   Called by miniR's Rust side after converting the .Call result to RValue.
   Exported so Rust can dlsym("_minir_free_allocs") on the .so. */
void _minir_free_allocs(void) {
    /* Free data buffers, then the SEXPREC structs.
       Note: STRSXP/VECSXP contain pointers to other SEXPs that are also
       in the alloc list, so we only free the data pointer (array of SEXP*),
       not recursively — each child SEXP gets freed when its own alloc_list
       entry is processed. */
    for (int i = 0; i < _minir_alloc_count; i++) {
        SEXP s = _minir_alloc_list[i];
        if (s && s != R_NilValue) {
            _minir_free_sexp_data(s);
        }
    }
    for (int i = 0; i < _minir_alloc_count; i++) {
        SEXP s = _minir_alloc_list[i];
        if (s && s != R_NilValue) {
            free(s);
        }
    }
    _minir_alloc_count = 0;
    _minir_protect_count = 0;
    _minir_has_error = 0;
    _minir_error_msg[0] = '\0';
}

/* ── Misc stubs ── */

/* Rf_duplicate — shallow copy */
static inline SEXP Rf_duplicate(SEXP x) {
    if (x == R_NilValue || Rf_isNull(x)) return R_NilValue;
    SEXP out = Rf_allocVector(TYPEOF(x), LENGTH(x));
    if (LENGTH(x) > 0 && x->data && out->data) {
        size_t elem_size = 0;
        switch (TYPEOF(x)) {
            case REALSXP: elem_size = sizeof(double); break;
            case INTSXP: case LGLSXP: elem_size = sizeof(int); break;
            case RAWSXP: elem_size = sizeof(Rbyte); break;
            case CPLXSXP: elem_size = sizeof(Rcomplex); break;
            case STRSXP: case VECSXP: elem_size = sizeof(SEXP); break;
            default: break;
        }
        if (elem_size > 0) {
            memcpy(out->data, x->data, (size_t)LENGTH(x) * elem_size);
        }
    }
    return out;
}
#define duplicate   Rf_duplicate
#define Rf_lazy_duplicate Rf_duplicate
#define lazy_duplicate    Rf_duplicate

/* Rf_inherits — stub */
static inline Rboolean Rf_inherits(SEXP x, const char *name) {
    (void)x; (void)name;
    return FALSE;
}
#define inherits Rf_inherits

/* Rf_translateCharUTF8 — identity for miniR (we use UTF-8 internally) */
#define Rf_translateCharUTF8(x)  R_CHAR(x)
#define translateCharUTF8        Rf_translateCharUTF8

/* Rf_install — symbol creation stub (returns R_NilValue) */
static inline SEXP Rf_install(const char *name) {
    (void)name;
    return R_NilValue;
}
#define install Rf_install

/* R_do_slot — S4 slot access stub */
static inline SEXP R_do_slot(SEXP obj, SEXP name) {
    (void)obj; (void)name;
    return R_NilValue;
}

/* Rf_nrows / Rf_ncols — dimension helpers */
static inline int Rf_nrows(SEXP x) { return LENGTH(x); }
static inline int Rf_ncols(SEXP x) { (void)x; return 1; }
#define nrows Rf_nrows
#define ncols Rf_ncols

/* R_CheckUserInterrupt — no-op */
static inline void R_CheckUserInterrupt(void) {}

/* Rf_PrintValue — no-op stub */
#define Rf_PrintValue(x)  ((void)(x))
#define PrintValue        Rf_PrintValue

/* ── R_NaString ── */
static struct SEXPREC _R_NaString_rec = { CHARSXP, 0, 0, 2, (void*)"NA", NULL };
#define R_NaString  (&_R_NaString_rec)

/* R_BlankString */
static struct SEXPREC _R_BlankString_rec = { CHARSXP, 0, 0, 0, (void*)"", NULL };
#define R_BlankString (&_R_BlankString_rec)

#ifdef __cplusplus
}
#endif

#endif /* MINIR_RINTERNALS_H */
