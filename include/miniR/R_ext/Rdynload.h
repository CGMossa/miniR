/*
 * miniR — R_ext/Rdynload.h
 *
 * Compatibility header for packages that include <R_ext/Rdynload.h>
 * for R_registerRoutines and related types.
 * Everything is already defined in Rinternals.h.
 */

#ifndef MINIR_R_EXT_RDYNLOAD_H
#define MINIR_R_EXT_RDYNLOAD_H

#ifdef __cplusplus
extern "C" {
#endif

#include "../Rinternals.h"

/* DL_FUNC already defined in Rinternals.h */


#ifdef __cplusplus
}
#endif
#endif /* MINIR_R_EXT_RDYNLOAD_H */
