/*
 * miniR — Rinternals.h
 *
 * C-compatible header for R packages compiled against miniR.
 * Defines the SEXP type, accessor macros, and API function declarations.
 *
 * All state and function implementations live in minir_runtime.c, which
 * is compiled once per package .so. This header only contains struct
 * definitions, macros, constants, and extern declarations.
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

/*
 * Pairlist data — used by LISTSXP/LANGSXP nodes for CAR/CDR/TAG.
 * Stored at the address pointed to by data when stype is LISTSXP or LANGSXP.
 */
typedef struct minir_pairlist_data {
    struct SEXPREC *car;
    struct SEXPREC *cdr;
    struct SEXPREC *tag;
} minir_pairlist_data;

struct SEXPREC {
    uint8_t  type;      /* SEXPTYPE tag */
    uint8_t  flags;     /* GC mark, named count (reserved) */
    uint16_t padding;
    int32_t  length;    /* vector length (or 0 for scalars/pairlists) */
    void    *data;      /* type-dependent data pointer */
    struct SEXPREC *attrib; /* attributes pairlist (or NULL) */
};

typedef struct SEXPREC *SEXP;

/* ── NA values ── */

static inline double R_NaReal(void) {
    union { uint64_t u; double d; } na;
    na.u = 0x7FF00000000007A2ULL;
    return na.d;
}
#define NA_REAL     R_NaReal()
#define NA_INTEGER  (-2147483647 - 1)
#define NA_LOGICAL  NA_INTEGER
#define R_NaInt     NA_INTEGER

static inline int R_IsNA(double x) {
    union { double d; uint64_t u; } val;
    val.d = x;
    return val.u == 0x7FF00000000007A2ULL;
}
#define ISNA(x)  R_IsNA(x)
#define ISNAN(x) (isnan(x))

/* ── Globals (defined in minir_runtime.c) ── */

extern SEXP R_NilValue;
extern SEXP R_NaString;
extern SEXP R_BlankString;
extern SEXP R_NamesSymbol;
extern SEXP R_DimSymbol;
extern SEXP R_DimNamesSymbol;
extern SEXP R_ClassSymbol;
extern SEXP R_RowNamesSymbol;
extern SEXP R_LevelsSymbol;

/* ── Accessor macros ── */

#define TYPEOF(x)    ((SEXPTYPE)((x)->type))
#define LENGTH(x)    ((x)->length)
#define XLENGTH(x)   ((R_xlen_t)((x)->length))

#define REAL(x)      ((double*)((x)->data))
#define INTEGER(x)   ((int*)((x)->data))
#define LOGICAL(x)   ((int*)((x)->data))
#define RAW(x)       ((Rbyte*)((x)->data))
#define COMPLEX(x)   ((Rcomplex*)((x)->data))

#define STRING_ELT(x, i)   (((SEXP*)((x)->data))[i])
#define VECTOR_ELT(x, i)   (((SEXP*)((x)->data))[i])
#define SET_STRING_ELT(x, i, v)  (((SEXP*)((x)->data))[i] = (v))
#define SET_VECTOR_ELT(x, i, v)  (((SEXP*)((x)->data))[i] = (v))

#define R_CHAR(x)    ((const char*)((x)->data))
#define CHAR(x)      R_CHAR(x)

/* SETLENGTH — resize a vector (only shrinking is safe without realloc) */
#define SETLENGTH(x, n) ((x)->length = (int32_t)(n))
#define SET_TRUELENGTH(x, n) ((void)(n))

/* Pairlist accessors (LISTSXP / LANGSXP) */
#define CAR(x)   (((minir_pairlist_data*)((x)->data))->car)
#define CDR(x)   (((minir_pairlist_data*)((x)->data))->cdr)
#define TAG(x)   (((minir_pairlist_data*)((x)->data))->tag)
#define SETCAR(x, v)  (CAR(x) = (v))
#define SETCDR(x, v)  (CDR(x) = (v))
#define SET_TAG(x, v) (TAG(x) = (v))

/* ── Function declarations (implemented in minir_runtime.c) ── */

/* Allocation */
SEXP Rf_allocVector(SEXPTYPE type, R_xlen_t length);
SEXP Rf_allocMatrix(SEXPTYPE type, int nrow, int ncol);
char *R_alloc(size_t nelem, int eltsize);
SEXP Rf_ScalarReal(double x);
SEXP Rf_ScalarInteger(int x);
SEXP Rf_ScalarLogical(int x);
SEXP Rf_ScalarString(SEXP x);

/* Strings */
SEXP Rf_mkChar(const char *str);
SEXP Rf_mkCharLen(const char *str, int len);
SEXP Rf_mkString(const char *str);

/* Symbols */
SEXP Rf_install(const char *name);

/* Pairlists */
SEXP Rf_cons(SEXP car, SEXP cdr);
SEXP Rf_lcons(SEXP car, SEXP cdr);

/* Protection */
SEXP Rf_protect(SEXP s);
void Rf_unprotect(int n);

/* Type checking */
Rboolean Rf_isNull(SEXP x);
Rboolean Rf_isReal(SEXP x);
Rboolean Rf_isInteger(SEXP x);
Rboolean Rf_isLogical(SEXP x);
Rboolean Rf_isString(SEXP x);
Rboolean Rf_isVector(SEXP x);
Rboolean Rf_inherits(SEXP x, const char *name);

/* Attributes */
SEXP Rf_getAttrib(SEXP x, SEXP name);
SEXP Rf_setAttrib(SEXP x, SEXP name, SEXP val);

/* Coercion */
double Rf_asReal(SEXP x);
int Rf_asInteger(SEXP x);
int Rf_asLogical(SEXP x);
SEXP Rf_coerceVector(SEXP x, SEXPTYPE type);

/* Duplication */
SEXP Rf_duplicate(SEXP x);

/* Error handling */
void Rf_error(const char *fmt, ...) __attribute__((noreturn));
void Rf_warning(const char *fmt, ...);

/* Output */
void Rprintf(const char *fmt, ...);
void REprintf(const char *fmt, ...);

/* Dimensions */
int Rf_nrows(SEXP x);
int Rf_ncols(SEXP x);

/* Misc */
void R_CheckUserInterrupt(void);
SEXP R_do_slot(SEXP obj, SEXP name);

/* ── R_RegisterRoutines ── */

typedef struct {
    const char *name;
    void *fun;  /* DL_FUNC */
    int numArgs;
} R_CallMethodDef;

typedef struct {
    const char *name;
    void *fun;
    int numArgs;
} R_CMethodDef;

typedef void *R_FortranMethodDef;
typedef void *R_ExternalMethodDef;

typedef struct _DllInfo DllInfo;

int R_registerRoutines(DllInfo *info,
                       const R_CMethodDef *cMethods,
                       const R_CallMethodDef *callMethods,
                       const R_FortranMethodDef *fortranMethods,
                       const R_ExternalMethodDef *externalMethods);

void R_useDynamicSymbols(DllInfo *info, Rboolean value);
void R_forceSymbols(DllInfo *info, Rboolean value);

/* DllInfo accessor — packages get this from R_init_<pkgname>(DllInfo *info) */
extern DllInfo *_minir_current_dll_info;

/* ── Protected call trampoline (called by Rust) ── */

typedef SEXP (*_minir_dotcall_fn)();

/*
 * Call a native function with setjmp error protection.
 * Returns 0 on success, 1 if Rf_error was called.
 * On error, call _minir_get_error_msg() for the message.
 */
int _minir_call_protected(_minir_dotcall_fn fn, SEXP *args, int nargs, SEXP *result);
const char *_minir_get_error_msg(void);
int _minir_has_error_flag(void);

/* Free all tracked allocations (called by Rust after .Call) */
void _minir_free_allocs(void);

/* Get registered .Call methods */
typedef struct {
    const char *name;
    void *fun;
    int numArgs;
} _minir_registered_call;

int _minir_get_registered_calls(_minir_registered_call **out);

/* ── Convenience macros (GNU R compat) ── */

#define allocVector     Rf_allocVector
#define allocVector3    Rf_allocVector
#define allocMatrix     Rf_allocMatrix
#define ScalarReal      Rf_ScalarReal
#define ScalarInteger   Rf_ScalarInteger
#define ScalarLogical   Rf_ScalarLogical
#define ScalarString    Rf_ScalarString
#define mkChar          Rf_mkChar
#define mkCharLen       Rf_mkCharLen
#define mkString        Rf_mkString
#define install         Rf_install
#define protect         Rf_protect
#define PROTECT(s)      Rf_protect(s)
#define UNPROTECT(n)    Rf_unprotect(n)
#define R_PreserveObject(x) ((void)(x))
#define R_ReleaseObject(x)  ((void)(x))
#define isNull          Rf_isNull
#define isReal          Rf_isReal
#define isInteger       Rf_isInteger
#define isLogical       Rf_isLogical
#define isString        Rf_isString
#define isVector        Rf_isVector
#define inherits        Rf_inherits
#define getAttrib       Rf_getAttrib
#define setAttrib       Rf_setAttrib
#define asReal          Rf_asReal
#define asInteger       Rf_asInteger
#define asLogical       Rf_asLogical
#define coerceVector    Rf_coerceVector
#define duplicate       Rf_duplicate
#define Rf_lazy_duplicate Rf_duplicate
#define lazy_duplicate  Rf_duplicate
#define error           Rf_error
#define warning         Rf_warning
#define nrows           Rf_nrows
#define ncols           Rf_ncols
#define cons            Rf_cons
#define lcons           Rf_lcons
#define Rf_translateCharUTF8(x)  R_CHAR(x)
#define translateCharUTF8        Rf_translateCharUTF8
#define Rf_PrintValue(x)  ((void)(x))
#define PrintValue        Rf_PrintValue

#ifdef __cplusplus
}
#endif

#endif /* MINIR_RINTERNALS_H */
