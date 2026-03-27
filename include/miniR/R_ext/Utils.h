/* miniR — R_ext/Utils.h — utility functions */
#ifndef MINIR_R_EXT_UTILS_H
#define MINIR_R_EXT_UTILS_H

#include "../Rinternals.h"

/* R_CheckUserInterrupt — already in Rinternals.h */
/* R_rsort, R_isort, etc. — sorting utilities, stub as no-ops */

static inline void R_isort(int *x, int n) { (void)x; (void)n; }
static inline void R_rsort(double *x, int n) { (void)x; (void)n; }
static inline void R_csort(Rcomplex *x, int n) { (void)x; (void)n; }

#endif
