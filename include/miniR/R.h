/*
 * miniR — R.h
 *
 * Top-level header for R packages. Just includes Rinternals.h.
 * Some packages include <R.h> instead of <Rinternals.h>.
 */

#ifndef MINIR_R_H
#define MINIR_R_H

#include <limits.h>
#include "Rinternals.h"

/* R_ext/Boolean.h equivalent */
#ifndef R_EXT_BOOLEAN_H_
#define R_EXT_BOOLEAN_H_
/* Rboolean already defined in Rinternals.h */
#endif

/* Common R macros that some packages expect */
#ifndef R_INLINE
#define R_INLINE static inline
#endif

#endif /* MINIR_R_H */
