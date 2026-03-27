/*
 * miniR — R_ext/Rdynload.h
 *
 * Compatibility header for packages that include <R_ext/Rdynload.h>
 * for R_registerRoutines and related types.
 * Everything is already defined in Rinternals.h.
 */

#ifndef MINIR_R_EXT_RDYNLOAD_H
#define MINIR_R_EXT_RDYNLOAD_H

#include "../Rinternals.h"

typedef void (*DL_FUNC)();

#endif /* MINIR_R_EXT_RDYNLOAD_H */
