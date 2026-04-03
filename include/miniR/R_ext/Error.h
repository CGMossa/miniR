/* miniR — R_ext/Error.h — error/warning with call context */
#ifndef MINIR_R_EXT_ERROR_H
#define MINIR_R_EXT_ERROR_H

#ifdef __cplusplus
extern "C" {
#endif

#include "../Rinternals.h"

void Rf_errorcall(SEXP call, const char *fmt, ...);
void Rf_warningcall(SEXP call, const char *fmt, ...);

#define errorcall   Rf_errorcall
#define warningcall Rf_warningcall


#ifdef __cplusplus
}
#endif
#endif
