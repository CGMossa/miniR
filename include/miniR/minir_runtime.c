/*
 * miniR runtime — compiled once per package .so alongside the package's C files.
 *
 * Contains all global state (allocation tracking, protect stack, error handling)
 * and the implementations of the R C API functions declared in Rinternals.h.
 */

#include "Rinternals.h"

/* ════════════════════════════════════════════════════════════════════════════
 * Global state
 * ════════════════════════════════════════════════════════════════════════════ */

#define MINIR_MAX_ALLOCS  65536
#define MINIR_MAX_PROTECT 10000
#define MINIR_MAX_REGISTERED_CALLS 1024

static SEXP  _alloc_list[MINIR_MAX_ALLOCS];
static int   _alloc_count = 0;
static SEXP  _protect_stack[MINIR_MAX_PROTECT];
static int   _protect_count = 0;

static jmp_buf _error_jmp;
static char    _error_msg[4096];
static int     _has_error = 0;

/* Registered .Call methods from R_registerRoutines */
static _minir_registered_call _registered_calls[MINIR_MAX_REGISTERED_CALLS];
static int _registered_call_count = 0;

/* ════════════════════════════════════════════════════════════════════════════
 * Sentinel globals
 * ════════════════════════════════════════════════════════════════════════════ */

static struct SEXPREC _nil_rec   = { NILSXP,  0, 0, 0, NULL, NULL };
static struct SEXPREC _na_str    = { CHARSXP, 0, 0, 2, (void*)"NA", NULL };
static struct SEXPREC _blank_str = { CHARSXP, 0, 0, 0, (void*)"",  NULL };

SEXP R_NilValue   = &_nil_rec;
SEXP R_NaString   = &_na_str;
SEXP R_BlankString = &_blank_str;

/* Well-known symbol SEXPs */
static struct SEXPREC _sym_names    = { SYMSXP, 0, 0, 5, (void*)"names",    NULL };
static struct SEXPREC _sym_dim      = { SYMSXP, 0, 0, 3, (void*)"dim",      NULL };
static struct SEXPREC _sym_dimnames = { SYMSXP, 0, 0, 8, (void*)"dimnames", NULL };
static struct SEXPREC _sym_class    = { SYMSXP, 0, 0, 5, (void*)"class",    NULL };
static struct SEXPREC _sym_rownames = { SYMSXP, 0, 0, 10,(void*)"row.names",NULL };
static struct SEXPREC _sym_levels   = { SYMSXP, 0, 0, 6, (void*)"levels",   NULL };

SEXP R_NamesSymbol    = &_sym_names;
SEXP R_DimSymbol      = &_sym_dim;
SEXP R_DimNamesSymbol = &_sym_dimnames;
SEXP R_ClassSymbol    = &_sym_class;
SEXP R_RowNamesSymbol = &_sym_rownames;
SEXP R_LevelsSymbol   = &_sym_levels;

DllInfo *_minir_current_dll_info = NULL;

/* ════════════════════════════════════════════════════════════════════════════
 * Allocation tracking
 * ════════════════════════════════════════════════════════════════════════════ */

static void _track(SEXP s) {
    if (_alloc_count < MINIR_MAX_ALLOCS) {
        _alloc_list[_alloc_count++] = s;
    }
}

/* ════════════════════════════════════════════════════════════════════════════
 * Allocation functions
 * ════════════════════════════════════════════════════════════════════════════ */

SEXP Rf_allocVector(SEXPTYPE type, R_xlen_t length) {
    SEXP s = (SEXP)calloc(1, sizeof(struct SEXPREC));
    if (!s) return R_NilValue;
    s->type = (uint8_t)type;
    s->length = (int32_t)length;
    s->attrib = R_NilValue;

    if (length > 0) {
        size_t n = (size_t)length;
        switch (type) {
            case REALSXP:   s->data = calloc(n, sizeof(double));   break;
            case INTSXP:
            case LGLSXP:    s->data = calloc(n, sizeof(int));      break;
            case CPLXSXP:   s->data = calloc(n, sizeof(Rcomplex)); break;
            case STRSXP:
            case VECSXP:
            case EXPRSXP:   s->data = calloc(n, sizeof(SEXP));     break;
            case RAWSXP:    s->data = calloc(n, sizeof(Rbyte));    break;
            default:        s->data = NULL; break;
        }
    }
    _track(s);
    return s;
}

SEXP Rf_allocMatrix(SEXPTYPE type, int nrow, int ncol) {
    return Rf_allocVector(type, (R_xlen_t)nrow * ncol);
}

SEXP Rf_ScalarReal(double x) {
    SEXP s = Rf_allocVector(REALSXP, 1);
    REAL(s)[0] = x;
    return s;
}

SEXP Rf_ScalarInteger(int x) {
    SEXP s = Rf_allocVector(INTSXP, 1);
    INTEGER(s)[0] = x;
    return s;
}

SEXP Rf_ScalarLogical(int x) {
    SEXP s = Rf_allocVector(LGLSXP, 1);
    LOGICAL(s)[0] = x;
    return s;
}

SEXP Rf_ScalarString(SEXP x) {
    SEXP s = Rf_allocVector(STRSXP, 1);
    SET_STRING_ELT(s, 0, x);
    return s;
}

/* ════════════════════════════════════════════════════════════════════════════
 * String / char functions
 * ════════════════════════════════════════════════════════════════════════════ */

SEXP Rf_mkChar(const char *str) {
    size_t len = strlen(str);
    SEXP s = (SEXP)calloc(1, sizeof(struct SEXPREC));
    if (!s) return R_NilValue;
    s->type = CHARSXP;
    s->length = (int32_t)len;
    s->attrib = R_NilValue;
    char *buf = (char*)malloc(len + 1);
    if (buf) memcpy(buf, str, len + 1);
    s->data = buf;
    _track(s);
    return s;
}

SEXP Rf_mkCharLen(const char *str, int len) {
    SEXP s = (SEXP)calloc(1, sizeof(struct SEXPREC));
    if (!s) return R_NilValue;
    s->type = CHARSXP;
    s->length = (int32_t)len;
    s->attrib = R_NilValue;
    char *buf = (char*)malloc((size_t)len + 1);
    if (buf) { memcpy(buf, str, (size_t)len); buf[len] = '\0'; }
    s->data = buf;
    _track(s);
    return s;
}

SEXP Rf_mkString(const char *str) {
    SEXP s = Rf_allocVector(STRSXP, 1);
    SET_STRING_ELT(s, 0, Rf_mkChar(str));
    return s;
}

/* ════════════════════════════════════════════════════════════════════════════
 * Symbols
 * ════════════════════════════════════════════════════════════════════════════ */

/* Simple symbol interning — returns well-known symbols or allocates new ones */
SEXP Rf_install(const char *name) {
    /* Check well-known symbols first */
    if (strcmp(name, "names") == 0)     return R_NamesSymbol;
    if (strcmp(name, "dim") == 0)       return R_DimSymbol;
    if (strcmp(name, "dimnames") == 0)  return R_DimNamesSymbol;
    if (strcmp(name, "class") == 0)     return R_ClassSymbol;
    if (strcmp(name, "row.names") == 0) return R_RowNamesSymbol;
    if (strcmp(name, "levels") == 0)    return R_LevelsSymbol;

    /* Allocate a new symbol */
    size_t len = strlen(name);
    SEXP s = (SEXP)calloc(1, sizeof(struct SEXPREC));
    if (!s) return R_NilValue;
    s->type = SYMSXP;
    s->length = (int32_t)len;
    s->attrib = R_NilValue;
    char *buf = (char*)malloc(len + 1);
    if (buf) memcpy(buf, name, len + 1);
    s->data = buf;
    _track(s);
    return s;
}

/* ════════════════════════════════════════════════════════════════════════════
 * Pairlists
 * ════════════════════════════════════════════════════════════════════════════ */

SEXP Rf_cons(SEXP car, SEXP cdr) {
    SEXP s = (SEXP)calloc(1, sizeof(struct SEXPREC));
    if (!s) return R_NilValue;
    s->type = LISTSXP;
    s->attrib = R_NilValue;
    minir_pairlist_data *pd = (minir_pairlist_data*)calloc(1, sizeof(minir_pairlist_data));
    if (pd) { pd->car = car; pd->cdr = cdr; pd->tag = R_NilValue; }
    s->data = pd;
    _track(s);
    return s;
}

SEXP Rf_lcons(SEXP car, SEXP cdr) {
    SEXP s = Rf_cons(car, cdr);
    if (s != R_NilValue) s->type = LANGSXP;
    return s;
}

/* ════════════════════════════════════════════════════════════════════════════
 * PROTECT / UNPROTECT
 * ════════════════════════════════════════════════════════════════════════════ */

SEXP Rf_protect(SEXP s) {
    if (_protect_count < MINIR_MAX_PROTECT)
        _protect_stack[_protect_count++] = s;
    return s;
}

void Rf_unprotect(int n) {
    _protect_count -= n;
    if (_protect_count < 0) _protect_count = 0;
}

/* ════════════════════════════════════════════════════════════════════════════
 * Type checking
 * ════════════════════════════════════════════════════════════════════════════ */

Rboolean Rf_isNull(SEXP x)    { return TYPEOF(x) == NILSXP  ? TRUE : FALSE; }
Rboolean Rf_isReal(SEXP x)    { return TYPEOF(x) == REALSXP  ? TRUE : FALSE; }
Rboolean Rf_isInteger(SEXP x) { return TYPEOF(x) == INTSXP   ? TRUE : FALSE; }
Rboolean Rf_isLogical(SEXP x) { return TYPEOF(x) == LGLSXP   ? TRUE : FALSE; }
Rboolean Rf_isString(SEXP x)  { return TYPEOF(x) == STRSXP   ? TRUE : FALSE; }

Rboolean Rf_isVector(SEXP x) {
    SEXPTYPE t = TYPEOF(x);
    return (t == REALSXP || t == INTSXP || t == LGLSXP || t == STRSXP ||
            t == VECSXP || t == RAWSXP || t == CPLXSXP) ? TRUE : FALSE;
}

Rboolean Rf_inherits(SEXP x, const char *name) {
    SEXP klass = Rf_getAttrib(x, R_ClassSymbol);
    if (klass == R_NilValue || TYPEOF(klass) != STRSXP) return FALSE;
    for (int i = 0; i < LENGTH(klass); i++) {
        if (strcmp(R_CHAR(STRING_ELT(klass, i)), name) == 0) return TRUE;
    }
    return FALSE;
}

/* ════════════════════════════════════════════════════════════════════════════
 * Attributes (pairlist-based, like GNU R)
 * ════════════════════════════════════════════════════════════════════════════ */

/* Compare two symbol SEXPs by name */
static int _sym_eq(SEXP a, SEXP b) {
    if (a == b) return 1;
    if (!a || !b) return 0;
    if (TYPEOF(a) != SYMSXP || TYPEOF(b) != SYMSXP) return 0;
    if (!a->data || !b->data) return 0;
    return strcmp((const char*)a->data, (const char*)b->data) == 0;
}

SEXP Rf_getAttrib(SEXP x, SEXP name) {
    if (!x || x == R_NilValue) return R_NilValue;
    SEXP attr = x->attrib;
    while (attr && attr != R_NilValue && TYPEOF(attr) == LISTSXP) {
        if (_sym_eq(TAG(attr), name)) return CAR(attr);
        attr = CDR(attr);
    }
    return R_NilValue;
}

SEXP Rf_setAttrib(SEXP x, SEXP name, SEXP val) {
    if (!x || x == R_NilValue) return val;

    /* Search for existing attribute with this name */
    SEXP attr = x->attrib;
    while (attr && attr != R_NilValue && TYPEOF(attr) == LISTSXP) {
        if (_sym_eq(TAG(attr), name)) {
            SETCAR(attr, val);
            return val;
        }
        attr = CDR(attr);
    }

    /* Not found — prepend a new node */
    SEXP node = Rf_cons(val, x->attrib ? x->attrib : R_NilValue);
    SET_TAG(node, name);
    x->attrib = node;
    return val;
}

/* ════════════════════════════════════════════════════════════════════════════
 * Coercion helpers
 * ════════════════════════════════════════════════════════════════════════════ */

double Rf_asReal(SEXP x) {
    switch (TYPEOF(x)) {
        case REALSXP: return LENGTH(x) > 0 ? REAL(x)[0] : NA_REAL;
        case INTSXP:  return LENGTH(x) > 0 ? (INTEGER(x)[0] == NA_INTEGER ? NA_REAL : (double)INTEGER(x)[0]) : NA_REAL;
        case LGLSXP:  return LENGTH(x) > 0 ? (LOGICAL(x)[0] == NA_LOGICAL ? NA_REAL : (double)LOGICAL(x)[0]) : NA_REAL;
        default:      return NA_REAL;
    }
}

int Rf_asInteger(SEXP x) {
    switch (TYPEOF(x)) {
        case INTSXP:  return LENGTH(x) > 0 ? INTEGER(x)[0] : NA_INTEGER;
        case REALSXP: return LENGTH(x) > 0 ? (R_IsNA(REAL(x)[0]) ? NA_INTEGER : (int)REAL(x)[0]) : NA_INTEGER;
        case LGLSXP:  return LENGTH(x) > 0 ? LOGICAL(x)[0] : NA_INTEGER;
        default:      return NA_INTEGER;
    }
}

int Rf_asLogical(SEXP x) {
    switch (TYPEOF(x)) {
        case LGLSXP:  return LENGTH(x) > 0 ? LOGICAL(x)[0] : NA_LOGICAL;
        case INTSXP:  return LENGTH(x) > 0 ? (INTEGER(x)[0] == NA_INTEGER ? NA_LOGICAL : (INTEGER(x)[0] != 0)) : NA_LOGICAL;
        case REALSXP: return LENGTH(x) > 0 ? (R_IsNA(REAL(x)[0]) ? NA_LOGICAL : (REAL(x)[0] != 0.0)) : NA_LOGICAL;
        default:      return NA_LOGICAL;
    }
}

SEXP Rf_coerceVector(SEXP x, SEXPTYPE type) {
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
            default: break;
        }
    }
    UNPROTECT(1);
    return out;
}

/* ════════════════════════════════════════════════════════════════════════════
 * Duplication
 * ════════════════════════════════════════════════════════════════════════════ */

SEXP Rf_duplicate(SEXP x) {
    if (!x || x == R_NilValue) return R_NilValue;
    SEXP out = Rf_allocVector(TYPEOF(x), LENGTH(x));
    if (LENGTH(x) > 0 && x->data && out->data) {
        size_t elem_size = 0;
        switch (TYPEOF(x)) {
            case REALSXP:  elem_size = sizeof(double); break;
            case INTSXP:
            case LGLSXP:   elem_size = sizeof(int); break;
            case RAWSXP:   elem_size = sizeof(Rbyte); break;
            case CPLXSXP:  elem_size = sizeof(Rcomplex); break;
            case STRSXP:
            case VECSXP:   elem_size = sizeof(SEXP); break;
            default: break;
        }
        if (elem_size > 0)
            memcpy(out->data, x->data, (size_t)LENGTH(x) * elem_size);
    }
    /* Copy attributes */
    out->attrib = x->attrib;
    return out;
}

/* ════════════════════════════════════════════════════════════════════════════
 * Error handling
 * ════════════════════════════════════════════════════════════════════════════ */

void Rf_error(const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(_error_msg, sizeof(_error_msg), fmt, ap);
    va_end(ap);
    _has_error = 1;
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

/* ════════════════════════════════════════════════════════════════════════════
 * Dimensions / misc
 * ════════════════════════════════════════════════════════════════════════════ */

int Rf_nrows(SEXP x) {
    SEXP dim = Rf_getAttrib(x, R_DimSymbol);
    if (dim != R_NilValue && TYPEOF(dim) == INTSXP && LENGTH(dim) >= 1)
        return INTEGER(dim)[0];
    return LENGTH(x);
}

int Rf_ncols(SEXP x) {
    SEXP dim = Rf_getAttrib(x, R_DimSymbol);
    if (dim != R_NilValue && TYPEOF(dim) == INTSXP && LENGTH(dim) >= 2)
        return INTEGER(dim)[1];
    return 1;
}

void R_CheckUserInterrupt(void) { /* no-op */ }

SEXP R_do_slot(SEXP obj, SEXP name) {
    return Rf_getAttrib(obj, name);
}

/* ════════════════════════════════════════════════════════════════════════════
 * R_registerRoutines
 * ════════════════════════════════════════════════════════════════════════════ */

int R_registerRoutines(DllInfo *info,
                       const R_CMethodDef *cMethods,
                       const R_CallMethodDef *callMethods,
                       const R_FortranMethodDef *fortranMethods,
                       const R_ExternalMethodDef *externalMethods)
{
    (void)info;
    (void)cMethods;      /* .C methods — not supported yet */
    (void)fortranMethods;
    (void)externalMethods;

    if (callMethods) {
        for (int i = 0; callMethods[i].name != NULL; i++) {
            if (_registered_call_count < MINIR_MAX_REGISTERED_CALLS) {
                _registered_calls[_registered_call_count].name = callMethods[i].name;
                _registered_calls[_registered_call_count].fun = callMethods[i].fun;
                _registered_calls[_registered_call_count].numArgs = callMethods[i].numArgs;
                _registered_call_count++;
            }
        }
    }
    return 1;
}

void R_useDynamicSymbols(DllInfo *info, Rboolean value) {
    (void)info; (void)value;
}

void R_forceSymbols(DllInfo *info, Rboolean value) {
    (void)info; (void)value;
}

int _minir_get_registered_calls(_minir_registered_call **out) {
    *out = _registered_calls;
    return _registered_call_count;
}

/* ════════════════════════════════════════════════════════════════════════════
 * Protected call trampoline
 *
 * Called by Rust instead of invoking the native function directly.
 * Sets up setjmp so that Rf_error() longjmps back here safely.
 * Supports up to 65 SEXP arguments (R's .Call limit).
 * ════════════════════════════════════════════════════════════════════════════ */

int _minir_call_protected(_minir_dotcall_fn fn, SEXP *args, int nargs, SEXP *result) {
    _has_error = 0;
    _error_msg[0] = '\0';

    if (setjmp(_error_jmp) != 0) {
        *result = R_NilValue;
        return 1;  /* error — call _minir_get_error_msg() */
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
            /* 17-65 args: extremely rare, use a generic fallback */
            snprintf(_error_msg, sizeof(_error_msg),
                     ".Call with %d arguments is not supported (max 16)", nargs);
            _has_error = 1;
            *result = R_NilValue;
            return 1;
    }
    #undef A
    return 0;
}

const char *_minir_get_error_msg(void) {
    return _error_msg;
}

int _minir_has_error_flag(void) {
    return _has_error;
}

/* ════════════════════════════════════════════════════════════════════════════
 * Cleanup — free all tracked allocations
 * ════════════════════════════════════════════════════════════════════════════ */

void _minir_free_allocs(void) {
    /* First pass: free data buffers */
    for (int i = 0; i < _alloc_count; i++) {
        SEXP s = _alloc_list[i];
        if (s && s != R_NilValue && s->data) {
            free(s->data);
            s->data = NULL;
        }
    }
    /* Second pass: free SEXPREC structs */
    for (int i = 0; i < _alloc_count; i++) {
        SEXP s = _alloc_list[i];
        if (s && s != R_NilValue) {
            free(s);
        }
    }
    _alloc_count = 0;
    _protect_count = 0;
    _has_error = 0;
    _error_msg[0] = '\0';
}
