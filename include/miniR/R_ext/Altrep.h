/* miniR — R_ext/Altrep.h — ALTREP (alternative representation) stubs.
 * ALTREP is GNU R's mechanism for lazy/compact vector representations.
 * miniR doesn't implement ALTREP but provides this header so packages
 * that conditionally use it can compile. */
#ifndef MINIR_R_EXT_ALTREP_H
#define MINIR_R_EXT_ALTREP_H

#include "../Rinternals.h"

/* ALTREP class registration — no-ops in miniR */
typedef void *R_altrep_class_t;

#define R_make_altreal_class(n, p, i)   ((R_altrep_class_t)NULL)
#define R_make_altinteger_class(n, p, i) ((R_altrep_class_t)NULL)
#define R_make_altlogical_class(n, p, i) ((R_altrep_class_t)NULL)
#define R_make_altstring_class(n, p, i)  ((R_altrep_class_t)NULL)
#define R_make_altraw_class(n, p, i)     ((R_altrep_class_t)NULL)

/* ALTREP method setters — all no-ops */
#define R_set_altrep_Length_method(c, m)     ((void)0)
#define R_set_altrep_Inspect_method(c, m)    ((void)0)
#define R_set_altrep_Duplicate_method(c, m)  ((void)0)
#define R_set_altrep_Coerce_method(c, m)     ((void)0)
#define R_set_altrep_Serialized_state_method(c, m) ((void)0)
#define R_set_altrep_Unserialize_method(c, m)      ((void)0)

#define R_set_altreal_Elt_method(c, m)       ((void)0)
#define R_set_altreal_Get_region_method(c, m) ((void)0)
#define R_set_altinteger_Elt_method(c, m)    ((void)0)
#define R_set_altinteger_Get_region_method(c, m) ((void)0)
#define R_set_altlogical_Elt_method(c, m)    ((void)0)
#define R_set_altstring_Elt_method(c, m)     ((void)0)
#define R_set_altvec_Dataptr_method(c, m)    ((void)0)
#define R_set_altvec_Dataptr_or_null_method(c, m) ((void)0)
#define R_set_altinteger_No_NA_method(c, m)  ((void)0)
#define R_set_altreal_No_NA_method(c, m)     ((void)0)
#define R_set_altlogical_No_NA_method(c, m)  ((void)0)
#define R_set_altinteger_Is_sorted_method(c, m) ((void)0)
#define R_set_altreal_Is_sorted_method(c, m) ((void)0)
#define R_set_altinteger_Sum_method(c, m)    ((void)0)
#define R_set_altreal_Sum_method(c, m)       ((void)0)
#define R_set_altinteger_Min_method(c, m)    ((void)0)
#define R_set_altinteger_Max_method(c, m)    ((void)0)
#define R_set_altreal_Min_method(c, m)       ((void)0)
#define R_set_altreal_Max_method(c, m)       ((void)0)

/* Sorted constants */
#define SORTED_INCR 1
#define SORTED_DECR -1
#define SORTED_EQUAL 0
#define KNOWN_NA_1ST 1
#define KNOWN_NO_NA 0

/* ALTREP queries — always return false/standard behavior */
#define ALTREP(x)               0
#define ALTVEC_DATAPTR(x)       ((x)->data)
#define ALTVEC_DATAPTR_RO(x)    ((const void*)((x)->data))

/* ALTREP instance creation — returns a normal vector (no ALTREP) */
R_INLINE SEXP R_new_altrep(R_altrep_class_t cls, SEXP data1, SEXP data2) {
    (void)cls; (void)data2;
    return data1 ? data1 : R_NilValue;
}

/* ALTREP data accessors — stubs */
R_INLINE SEXP R_altrep_data1(SEXP x) { (void)x; return R_NilValue; }
R_INLINE SEXP R_altrep_data2(SEXP x) { (void)x; return R_NilValue; }
R_INLINE void R_set_altrep_data1(SEXP x, SEXP v) { (void)x; (void)v; }
R_INLINE void R_set_altrep_data2(SEXP x, SEXP v) { (void)x; (void)v; }

#endif /* MINIR_R_EXT_ALTREP_H */
