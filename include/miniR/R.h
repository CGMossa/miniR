/*
 * miniR — R.h
 *
 * Top-level header for R packages. Just includes Rinternals.h.
 * Some packages include <R.h> instead of <Rinternals.h>.
 */

#ifndef MINIR_R_H
#define MINIR_R_H

#include <limits.h>
#include <float.h>
#include <stdint.h>
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

/* Fortran name mangling — must be available from R.h */
#ifndef F77_NAME
#define F77_NAME(x) x ## _
#endif
#ifndef F77_CALL
#define F77_CALL(x) x ## _
#endif
#ifndef F77_SUB
#define F77_SUB(x) x ## _
#endif

/* Noreturn attribute — used by RSQLite and other packages */
#ifndef NORET
#if defined(__GNUC__) || defined(__clang__)
#define NORET __attribute__((noreturn))
#elif defined(_MSC_VER)
#define NORET __declspec(noreturn)
#else
#define NORET
#endif
#endif

#endif /* MINIR_R_H */
