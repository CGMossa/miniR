/* miniR — R_ext/Arith.h — arithmetic macros and constants */
#ifndef MINIR_R_EXT_ARITH_H
#define MINIR_R_EXT_ARITH_H

#ifdef __cplusplus
extern "C" {
#endif

#include <math.h>
#include <float.h>

#define R_PosInf    INFINITY
#define R_NegInf    (-INFINITY)
#define R_NaN       NAN
#define R_FINITE(x) isfinite(x)
#define R_IsNaN(x)  isnan(x)

/* R_IsNA and ISNA are already defined in Rinternals.h with the correct
   NaN-payload check. These are fallback definitions for files that
   include only Arith.h. */
#ifndef ISNA
#define ISNA(x)     isnan(x)
#define R_IsNA(x)   isnan(x)
#endif


#ifdef __cplusplus
}
#endif
#endif /* MINIR_R_EXT_ARITH_H */
