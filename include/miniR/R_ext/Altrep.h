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

/* ALTREP queries — always return false/standard behavior */
#define ALTREP(x)               0
#define ALTVEC_DATAPTR(x)       ((x)->data)
#define ALTVEC_DATAPTR_RO(x)    ((const void*)((x)->data))

#endif /* MINIR_R_EXT_ALTREP_H */
