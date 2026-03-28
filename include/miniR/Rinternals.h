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
#include <limits.h>
#include <float.h>

/* R_INLINE — used by package headers */
#ifndef R_INLINE
#define R_INLINE static inline
#endif

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
extern SEXP R_GlobalEnv;
extern SEXP R_BaseEnv;
extern SEXP R_UnboundValue;
extern SEXP R_EmptyEnv;
extern SEXP R_MissingArg;
extern SEXP R_NamesSymbol;
extern SEXP R_DimSymbol;
extern SEXP R_DimNamesSymbol;
extern SEXP R_ClassSymbol;
extern SEXP R_RowNamesSymbol;
extern SEXP R_LevelsSymbol;
extern SEXP R_DotsSymbol;

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

/* Read-only data pointers (R 4.0+) */
#define STRING_PTR_RO(x)  ((const SEXP*)((x)->data))
#define INTEGER_RO(x)     ((const int*)((x)->data))
#define REAL_RO(x)        ((const double*)((x)->data))
#define LOGICAL_RO(x)     ((const int*)((x)->data))
#define RAW_RO(x)         ((const Rbyte*)((x)->data))
#define COMPLEX_RO(x)     ((const Rcomplex*)((x)->data))
#define DATAPTR_RO(x)     ((const void*)((x)->data))
#define RAW_POINTER(x)    RAW(x)

/* REAL_ELT / INTEGER_ELT — single-element accessors */
#define REAL_ELT(x, i)    (REAL(x)[i])
#define INTEGER_ELT(x, i) (INTEGER(x)[i])
#define LOGICAL_ELT(x, i) (LOGICAL(x)[i])

/* Data pointer access */
#define DATAPTR_OR_NULL(x) ((x)->data)
#define DATAPTR(x)         ((x)->data)

/* Region-based element access (ALTREP compat) */
static inline int INTEGER_GET_REGION(SEXP x, int i, int n, int *buf) {
    int len = LENGTH(x);
    int actual = (i + n > len) ? len - i : n;
    if (actual > 0) memcpy(buf, INTEGER(x) + i, (size_t)actual * sizeof(int));
    return actual;
}
static inline int REAL_GET_REGION(SEXP x, int i, int n, double *buf) {
    int len = LENGTH(x);
    int actual = (i + n > len) ? len - i : n;
    if (actual > 0) memcpy(buf, REAL(x) + i, (size_t)actual * sizeof(double));
    return actual;
}

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

/* ── Character encoding ── */

typedef enum { CE_NATIVE = 0, CE_UTF8 = 1, CE_LATIN1 = 2, CE_BYTES = 3, CE_SYMBOL = 5, CE_ANY = 99 } cetype_t;

/* NA_STRING — canonical NA for character vectors */
#define NA_STRING R_NaString

/* ── External pointer API ── */

SEXP R_MakeExternalPtr(void *p, SEXP tag, SEXP prot);
void *R_ExternalPtrAddr(SEXP s);
SEXP R_ExternalPtrTag(SEXP s);
SEXP R_ExternalPtrProtected(SEXP s);
void R_ClearExternalPtr(SEXP s);
void R_SetExternalPtrAddr(SEXP s, void *p);
void R_RegisterCFinalizer(SEXP s, void (*fun)(SEXP));
void R_RegisterCFinalizerEx(SEXP s, void (*fun)(SEXP), Rboolean onexit);

/* ── Function declarations (implemented in minir_runtime.c) ── */

/* Allocation */
SEXP Rf_allocVector(SEXPTYPE type, R_xlen_t length);
SEXP Rf_allocMatrix(SEXPTYPE type, int nrow, int ncol);
char *R_alloc(size_t nelem, int eltsize);
SEXP Rf_ScalarReal(double x);
SEXP Rf_ScalarInteger(int x);
SEXP Rf_ScalarLogical(int x);
SEXP Rf_ScalarString(SEXP x);

/* Length */
R_len_t Rf_length(SEXP x);
R_xlen_t Rf_xlength(SEXP x);
SEXP Rf_lengthgets(SEXP x, R_len_t n);
SEXP Rf_xlengthgets(SEXP x, R_xlen_t n);
#define lengthgets Rf_lengthgets
#define xlength Rf_xlength
#define xlengthgets Rf_xlengthgets

/* Strings */
SEXP Rf_mkChar(const char *str);
SEXP Rf_mkCharLen(const char *str, int len);
SEXP Rf_mkCharCE(const char *str, cetype_t encoding);
cetype_t Rf_getCharCE(SEXP x);
Rboolean Rf_StringBlank(SEXP x);
SEXP Rf_mkString(const char *str);
SEXP Rf_mkCharLenCE(const char *str, int len, cetype_t encoding);
const char *Rf_translateChar(SEXP x);
#define mkCharLenCE Rf_mkCharLenCE
#define translateChar Rf_translateChar

/* Transient memory allocation */
char *S_alloc(long nelem, int eltsize);
char *S_realloc(char *ptr, long new_size, long old_size, int eltsize);

/* RNG */
void GetRNGstate(void);
void PutRNGstate(void);
double unif_rand(void);

/* Attribute shortcuts */
SEXP Rf_classgets(SEXP x, SEXP klass);
SEXP Rf_namesgets(SEXP x, SEXP names);
SEXP Rf_dimgets(SEXP x, SEXP dim);
#define classgets Rf_classgets
#define namesgets Rf_namesgets
#define dimgets Rf_dimgets

/* MARK_NOT_MUTABLE — no-op in miniR */
void MARK_NOT_MUTABLE(SEXP x);
SEXP PRENV(SEXP x);

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

/* Error/warning with call (defined in csrc/native_trampoline.c) */
void Rf_errorcall(SEXP call, const char *fmt, ...) __attribute__((noreturn));
void Rf_warningcall(SEXP call, const char *fmt, ...);

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

/* Shallow copy */
SEXP Rf_shallow_duplicate(SEXP x);
#define shallow_duplicate Rf_shallow_duplicate
#define lazy_duplicate Rf_shallow_duplicate

/* Environment creation */
SEXP R_NewEnv(SEXP parent, int hash, int size);
void Rf_defineVar(SEXP sym, SEXP val, SEXP env);
#define defineVar Rf_defineVar

/* Closure internals (stubs) */
SEXP BODY(SEXP x);
SEXP CLOENV(SEXP x);
SEXP FORMALS(SEXP x);

/* Attribute direct access */
#define ATTRIB(x)       ((x)->attrib)
#define SET_ATTRIB(x,v) ((x)->attrib = (v))

/* Pairlist navigation extras */
#define CAAR(x)   CAR(CAR(x))
#define CDAR(x)   CDR(CAR(x))
#define SETCADR(x,v) SETCAR(CDR(x), (v))
#define SETCADDR(x,v) SETCAR(CDR(CDR(x)), (v))

/* Type checking extras */
Rboolean Rf_isObject(SEXP x);
#define isObject Rf_isObject

/* Type name/conversion */
SEXPTYPE Rf_str2type(const char *s);
#define str2type Rf_str2type

/* Scalar complex constructor */
SEXP Rf_ScalarComplex(Rcomplex c);
#define ScalarComplex Rf_ScalarComplex

/* Object comparison */
int R_compute_identical(SEXP x, SEXP y, int flags);

/* Environment internals */
SEXP ENCLOS(SEXP x);
#define FRAME(x) R_NilValue
#define HASHTAB(x) R_NilValue
int R_existsVarInFrame(SEXP env, SEXP sym);
Rboolean R_IsNamespaceEnv(SEXP env);
SEXP R_lsInternal3(SEXP env, Rboolean all, Rboolean sorted);
SEXP R_ClosureExpr(SEXP x);
SEXP R_ParentEnv(SEXP env);
void R_LockBinding(SEXP sym, SEXP env);
SEXP Rf_namesgets(SEXP x, SEXP names);
void SET_FRAME(SEXP x, SEXP v);
void SET_ENCLOS(SEXP x, SEXP v);
void SET_HASHTAB(SEXP x, SEXP v);
Rboolean R_BindingIsLocked(SEXP sym, SEXP env);
SEXP R_NamespaceEnvSpec(SEXP ns);
SEXP R_FindNamespace(SEXP name);
Rboolean R_IsPackageEnv(SEXP env);
SEXP R_PackageEnvName(SEXP env);

/* More stubs for rlang */
void R_CheckStack2(int extra);
void R_MakeActiveBinding(SEXP sym, SEXP fun, SEXP env);
SEXP R_MakeExternalPtrFn(void (*p)(void), SEXP tag, SEXP prot);
SEXP Rf_allocSExp(SEXPTYPE type);
R_xlen_t Rf_any_duplicated(SEXP x, Rboolean from_last);
int Rf_countContexts(int type, int subtype);
SEXP R_PromiseExpr(SEXP p);
SEXP R_ClosureFormals(SEXP x);
SEXP R_ClosureBody(SEXP x);
SEXP R_ClosureEnv(SEXP x);
Rboolean R_HasFnArgIdx(void);
SEXP R_FnArgIdx(int i);
void R_OrderVector1(int *indx, int n, SEXP x, Rboolean nalast, Rboolean decreasing);
Rboolean R_envHasNoSpecialSymbols(SEXP env);
void SET_PRENV(SEXP x, SEXP v);
void SET_PRCODE(SEXP x, SEXP v);
void SET_PRVALUE(SEXP x, SEXP v);
SEXP PRCODE(SEXP x);
SEXP PRVALUE(SEXP x);
#define allocSExp Rf_allocSExp
#define any_duplicated Rf_any_duplicated

/* More stubs */
SEXP Rf_installChar(SEXP x);
SEXP Rf_ScalarRaw(unsigned char x);
int R_EnvironmentIsLocked(SEXP env);
#define LEVELS(x) 0
#define SETLEVELS(x, v) ((void)(v))
#define installChar Rf_installChar
#define ScalarRaw Rf_ScalarRaw

/* ALTREP — always false in miniR */
#ifndef ALTREP
#define ALTREP(x) 0
#endif

/* Active bindings */
int R_BindingIsActive(SEXP sym, SEXP env);
SEXP R_ActiveBindingFunction(SEXP sym, SEXP env);
void Rf_onintr(void);
#define onintr Rf_onintr

/* More symbol constants */
extern SEXP R_BraceSymbol;
extern SEXP R_BracketSymbol;
extern SEXP R_Bracket2Symbol;
extern SEXP R_DoubleColonSymbol;
extern SEXP R_TripleColonSymbol;
extern int R_Interactive;

/* Weak references */
SEXP R_MakeWeakRef(SEXP key, SEXP val, SEXP fin, Rboolean onexit);
SEXP R_MakeWeakRefC(SEXP key, SEXP val, void (*fin)(SEXP), Rboolean onexit);
SEXP R_WeakRefKey(SEXP w);
SEXP R_WeakRefValue(SEXP w);

/* Duplicated detection */
SEXP Rf_duplicated(SEXP x, Rboolean from_last);
R_xlen_t Rf_any_duplicated3(SEXP x, SEXP incomp, Rboolean from_last);
#define duplicated Rf_duplicated

/* String encoding conversion */
const char *Rf_reEnc(const char *x, int ce_in, int ce_out, int subst);
const char *Rf_ucstoutf8(char *buf, unsigned int wc);

/* Closure modification */
void SET_BODY(SEXP x, SEXP v);
void SET_FORMALS(SEXP x, SEXP v);
void SET_CLOENV(SEXP x, SEXP v);

/* Globals */
extern SEXP R_NamespaceRegistry;
extern SEXP R_Srcref;
extern SEXP R_BaseNamespace;

/* S4 slot access */
#define GET_SLOT(x, name) Rf_getAttrib((x), (name))
#define SET_SLOT(x, name, val) Rf_setAttrib((x), (name), (val))

/* S4 class construction */
#define MAKE_CLASS(name) R_do_MAKE_CLASS(name)
SEXP R_do_MAKE_CLASS(const char *name);
#define NEW_OBJECT(cls) Rf_allocS4Object()

/* Dimension access */
#define GET_DIM(x) Rf_getAttrib((x), R_DimSymbol)
#define CONS(a, b) Rf_cons((a), (b))

/* Array allocation */
SEXP Rf_allocArray(SEXPTYPE type, SEXP dims);
#define allocArray Rf_allocArray

/* Variable lookup (stubs — return R_UnboundValue) */
SEXP Rf_findVar(SEXP sym, SEXP env);
SEXP Rf_findVarInFrame(SEXP env, SEXP sym);
SEXP Rf_findVarInFrame3(SEXP env, SEXP sym, int inherits_flag);
SEXP Rf_GetOption1(SEXP tag);
#define findVar Rf_findVar
#define findVarInFrame Rf_findVarInFrame
#define findVarInFrame3 Rf_findVarInFrame3
#define GetOption1 Rf_GetOption1

/* Eval variants */
SEXP R_tryEval(SEXP expr, SEXP env, int *errorOccurred);
SEXP R_tryEvalSilent(SEXP expr, SEXP env, int *errorOccurred);
int R_ToplevelExec(void (*fun)(void *), void *data);

/* Named list constructor */
SEXP Rf_mkNamed(SEXPTYPE type, const char **names);
#define mkNamed Rf_mkNamed

/* Type checking additions */
Rboolean Rf_isLanguage(SEXP x);
#define isLanguage Rf_isLanguage
Rboolean Rf_isSymbol(SEXP x);
#define isSymbol Rf_isSymbol

/* File path expansion */
const char *R_ExpandFileName(const char *fn);

/* Memory */
void *R_chk_calloc(size_t nelem, size_t elsize);
void *R_chk_realloc(void *ptr, size_t size);
void R_chk_free(void *ptr);

/* Removal from env frame */
void R_removeVarFromFrame(SEXP sym, SEXP env);

/* Type name for R_NativePrimitiveArgType */
typedef int R_NativePrimitiveArgType;

/* CDDR macro */
#define CDDR(x) CDR(CDR(x))

/* Growable vectors */
#define IS_GROWABLE(x) 0
#define SET_GROWABLE_BIT(x) ((void)0)

/* S4 allocation stub */
SEXP Rf_allocS4Object(void);
#define allocS4Object Rf_allocS4Object

/* ── R_RegisterRoutines ── */

typedef void (*DL_FUNC)();

typedef struct {
    const char *name;
    DL_FUNC fun;
    int numArgs;
} R_CallMethodDef;

typedef struct {
    const char *name;
    DL_FUNC fun;
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

/* Cross-package C function sharing */
void R_RegisterCCallable(const char *package, const char *name, DL_FUNC fptr);
DL_FUNC R_GetCCallable(const char *package, const char *name);

/* ── Serialization stubs (for packages like digest that need them) ── */

typedef void *R_pstream_data_t;
typedef enum { R_pstream_any_format = 0, R_pstream_ascii_format = 1, R_pstream_binary_format = 2, R_pstream_xdr_format = 3 } R_pstream_format_t;
typedef struct R_outpstream_st *R_outpstream_t;

struct R_outpstream_st {
    R_pstream_data_t data;
    R_pstream_format_t type;
    int version;
    void (*OutChar)(R_outpstream_t, int);
    void (*OutBytes)(R_outpstream_t, void *, int);
    SEXP (*OutPersistHookFunc)(SEXP, SEXP);
    SEXP OutPersistHookData;
};

/* Stubs — serialization from C is not supported in miniR */
static inline void R_InitOutPStream(R_outpstream_t s, R_pstream_data_t data,
    R_pstream_format_t type, int version,
    void (*outchar)(R_outpstream_t, int),
    void (*outbytes)(R_outpstream_t, void *, int),
    SEXP (*hook)(SEXP, SEXP), SEXP hookdata) {
    if (s) { s->data = data; s->type = type; s->version = version;
             s->OutChar = outchar; s->OutBytes = outbytes;
             s->OutPersistHookFunc = hook; s->OutPersistHookData = hookdata; }
}

static inline void R_Serialize(SEXP s, R_outpstream_t stream) {
    (void)s; (void)stream;
    /* No-op — serialization from C not supported. Packages that call this
       (e.g. digest's spooky hash) will get empty output. */
}

/* Rf_eval stub — evaluate an R expression from C.
   This is a deep R internal. In miniR, it's a no-op that returns R_NilValue.
   Packages that critically depend on Rf_eval from C won't work fully. */
SEXP Rf_eval(SEXP expr, SEXP env);

/* Rf_lcons — language node constructor */
SEXP Rf_lcons(SEXP car, SEXP cdr);

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
    DL_FUNC fun;
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
/* Note: lowercase `length` is NOT aliased because it conflicts with the
   struct field `s->length`. Use LENGTH() macro or Rf_length() function. */
#define mkChar          Rf_mkChar
#define mkCharLen       Rf_mkCharLen
#define mkCharCE        Rf_mkCharCE
#define getCharCE       Rf_getCharCE
#define StringBlank     Rf_StringBlank
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
#define Rf_lazy_duplicate Rf_shallow_duplicate
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
#define eval              Rf_eval

/* Arith constants (also in R_ext/Arith.h) */
#ifndef R_PosInf
#define R_PosInf   __builtin_inf()
#define R_NegInf   (-__builtin_inf())
#define R_NaN      __builtin_nan("")
#define R_FINITE(x) __builtin_isfinite(x)
#endif

/* Rf_list1..4 — allocate pairlists */
#define Rf_list1(a)       Rf_cons((a), R_NilValue)
#define Rf_list2(a,b)     Rf_cons((a), Rf_cons((b), R_NilValue))
#define Rf_list3(a,b,c)   Rf_cons((a), Rf_cons((b), Rf_cons((c), R_NilValue)))
#define list1 Rf_list1
#define list2 Rf_list2
#define list3 Rf_list3

/* length() alias — can't use #define because it conflicts with s->length.
   Use a static inline function instead. */
#ifndef R_NO_REMAP
static inline R_len_t length(SEXP x) { return Rf_length(x); }
#endif

/* R_LEN_T_MAX — maximum vector length */
#define R_LEN_T_MAX  INT32_MAX
#define R_XLEN_T_MAX ((R_xlen_t)INT64_MAX)

/* Scalar value accessors (R 4.0+) */
#define INTEGER_VALUE(x) (INTEGER(x)[0])
#define REAL_VALUE(x)    (REAL(x)[0])
#define LOGICAL_VALUE(x) (LOGICAL(x)[0])
#define STRING_VALUE(x)  R_CHAR(STRING_ELT((x), 0))

/* Type predicates — function-style aliases */
#define IS_RAW(x)       (TYPEOF(x) == RAWSXP)
#define IS_LOGICAL(x)   (TYPEOF(x) == LGLSXP)
#define IS_INTEGER(x)   (TYPEOF(x) == INTSXP)
#define IS_NUMERIC(x)   (TYPEOF(x) == REALSXP || TYPEOF(x) == INTSXP)
#define IS_CHARACTER(x)  (TYPEOF(x) == STRSXP)

/* Rf_asChar — coerce to CHARSXP */
static inline SEXP Rf_asChar(SEXP x) {
    if (TYPEOF(x) == STRSXP && LENGTH(x) > 0) return STRING_ELT(x, 0);
    return R_NaString;
}
#define asChar Rf_asChar

/* vmaxget / vmaxset — memory stack checkpoints (no-ops in miniR) */
#ifndef MINIR_VMAXGET_DEFINED
#define MINIR_VMAXGET_DEFINED
static inline void *vmaxget(void) { return (void*)0; }
static inline void vmaxset(void *p) { (void)p; }
#endif

/* Reference counting — always assume referenced (conservative) */
#define MAYBE_REFERENCED(x) 1
#define MAYBE_SHARED(x) 1
#define NO_REFERENCES(x) 0
#define NAMED(x) 2
#define SET_NAMED(x, v) ((void)(v))
#define warningcall       Rf_warningcall
#define errorcall         Rf_errorcall

/* Type checking aliases */
Rboolean Rf_isVectorAtomic(SEXP x);
Rboolean Rf_isVectorList(SEXP x);
Rboolean Rf_isMatrix(SEXP x);
Rboolean Rf_isNumeric(SEXP x);
Rboolean Rf_isFunction(SEXP x);
Rboolean Rf_isEnvironment(SEXP x);
#define isVectorAtomic Rf_isVectorAtomic
#define isVectorList   Rf_isVectorList
#define isMatrix       Rf_isMatrix
#define isNumeric      Rf_isNumeric
#define isFunction     Rf_isFunction
#define isEnvironment  Rf_isEnvironment

/* PROTECT_INDEX — indexed protection for reprotecting */
typedef int PROTECT_INDEX;
void R_ProtectWithIndex(SEXP s, PROTECT_INDEX *pi);
void R_Reprotect(SEXP s, PROTECT_INDEX i);
#define PROTECT_WITH_INDEX(x, i) R_ProtectWithIndex(x, i)
#define REPROTECT(x, i)         R_Reprotect(x, i)

/* Deep R internal stubs — needed by rlang/cli but not fully functional */
SEXP Rf_findVar(SEXP sym, SEXP env);
SEXP Rf_findVarInFrame3(SEXP env, SEXP sym, int inherits_flag);
SEXP PREXPR(SEXP x);
#define findVar Rf_findVar
#define findVarInFrame3 Rf_findVarInFrame3

/* R_ExecWithCleanup — execute with cleanup handler */
SEXP R_ExecWithCleanup(SEXP (*fun)(void *), void *data,
                       void (*cleanup)(void *), void *cleandata);
void *R_ExternalPtrAddrFn(SEXP s);

/* Pairlist navigation */
#define CADR(x)  CAR(CDR(x))
#define CADDR(x) CAR(CDR(CDR(x)))
#define CADDDR(x) CAR(CDR(CDR(CDR(x))))

/* Symbol name access */
#define PRINTNAME(x) (x)  /* In miniR, symbols store name as CHARSXP-like data */

/* Language object constructors */
SEXP Rf_lang1(SEXP s);
SEXP Rf_lang2(SEXP s, SEXP t);
SEXP Rf_lang3(SEXP s, SEXP t, SEXP u);
SEXP Rf_lang4(SEXP s, SEXP t, SEXP u, SEXP v);
#define lang1 Rf_lang1
#define lang2 Rf_lang2
#define lang3 Rf_lang3
#define lang4 Rf_lang4
SEXP Rf_lang5(SEXP s, SEXP t, SEXP u, SEXP v, SEXP w);
SEXP Rf_lang6(SEXP s, SEXP t, SEXP u, SEXP v, SEXP w, SEXP x);
#define lang5 Rf_lang5
#define lang6 Rf_lang6

/* findFun — find a function in an environment */
SEXP Rf_findFun(SEXP sym, SEXP env);
#define findFun Rf_findFun

/* LCONS — alias for Rf_lcons */
#define LCONS(a, b) Rf_lcons((a), (b))
#define Rf_list4(a,b,c,d) Rf_cons((a), Rf_cons((b), Rf_cons((c), Rf_cons((d), R_NilValue))))
#define list4 Rf_list4
#define R_IsNaN(x) isnan(x)
#define reEnc Rf_reEnc

/* More type predicates */
#define isFactor(x)   Rf_inherits((x), "factor")
#define isNewList(x)  (TYPEOF(x) == VECSXP)
Rboolean Rf_isFrame(SEXP x);
#define isFrame Rf_isFrame

/* Attribute copying */
void Rf_copyMostAttrib(SEXP from, SEXP to);
#define copyMostAttrib Rf_copyMostAttrib
#define SHALLOW_DUPLICATE_ATTRIB(from, to) Rf_copyMostAttrib((from), (to))
#define DUPLICATE_ATTRIB(from, to) Rf_copyMostAttrib((from), (to))

/* Vector NO_NA hints (R 4.0+ ALTREP) — always 0 (unknown) */
#define INTEGER_NO_NA(x) 0
#define REAL_NO_NA(x)    0
#define LOGICAL_NO_NA(x) 0
#define STRING_NO_NA(x)  0

/* TRUELENGTH — not used in miniR */
#define TRUELENGTH(x)     0
#define SET_TRUELENGTH(x,n) ((void)(n))

/* Pairlist traversal */
SEXP Rf_nthcdr(SEXP s, int n);
#define nthcdr Rf_nthcdr

/* Sorting */
void R_isort(int *x, int n);
void R_rsort(double *x, int n);
void iPsort(int *x, int n, int k);
void rPsort(double *x, int n, int k);
Rboolean Rf_isPrimitive(SEXP x);
#define isPrimitive Rf_isPrimitive

/* More type checks */
#define isOrdered(x)   Rf_inherits((x), "ordered")
#define isS4(x)        (TYPEOF(x) == OBJSXP || Rf_inherits((x), "refClass"))
#define isList(x)      (TYPEOF(x) == LISTSXP || TYPEOF(x) == NILSXP)
#define isPairList(x)  (TYPEOF(x) == LISTSXP)
#define isComplex(x)   (TYPEOF(x) == CPLXSXP)
#define isArray(x)     (Rf_getAttrib((x), R_DimSymbol) != R_NilValue)

/* Sorted hints (ALTREP) */
#define UNKNOWN_SORTEDNESS   INT_MIN
#define KNOWN_INCR(x)        0
#define KNOWN_DECR(x)        0
#define STRING_IS_SORTED(x)  0
#define REAL_IS_SORTED(x)    0
#define INTEGER_IS_SORTED(x) 0

/* More pairlist macros */
#define CDDDR(x)  CDR(CDR(CDR(x)))

/* XTRUELENGTH */
#define XTRUELENGTH(x)  0

/* Slot assignment */
void R_do_slot_assign(SEXP obj, SEXP name, SEXP val);

/* Console flush — no-op */
void R_FlushConsole(void);

/* allocList — allocate pairlist */
SEXP Rf_allocList(int n);
#define allocList Rf_allocList

/* Rf_match — match values */
SEXP Rf_match(SEXP table, SEXP x, int nomatch);
#define match Rf_match

/* Factor conversion */
SEXP Rf_asCharacterFactor(SEXP x);
#define asCharacterFactor Rf_asCharacterFactor

/* Rf_nchar — string length */
int Rf_nchar(SEXP x, int type, Rboolean allowNA, Rboolean keepNA, const char *msg_name);
#define nchar Rf_nchar

/* S_alloc — zeroed transient allocation */
char *S_alloc(long nelem, int eltsize);

/* Rf_type2char — SEXPTYPE to string */
const char *Rf_type2char(SEXPTYPE type);
#define type2char Rf_type2char
SEXP Rf_type2str(SEXPTYPE type);
#define type2str Rf_type2str

/* R_finite — finiteness check (function version of R_FINITE macro) */
int R_finite(double x);
#ifndef R_FINITE
#define R_FINITE(x) R_finite(x)
#endif

/* Memory allocation macros (also in R_ext/RS.h) */
#ifndef R_Calloc
#define R_Calloc(n, t)     ((t*)calloc((size_t)(n), sizeof(t)))
#define R_Free(p)          (free((void*)(p)), (p) = NULL)
#define R_Realloc(p, n, t) ((t*)realloc((void*)(p), (size_t)(n) * sizeof(t)))
#define Calloc(n, t)       R_Calloc(n, t)
#define Free(p)            R_Free(p)
#define Realloc(p, n, t)   R_Realloc(p, n, t)
#endif

#ifdef __cplusplus
}
#endif

#endif /* MINIR_RINTERNALS_H */
