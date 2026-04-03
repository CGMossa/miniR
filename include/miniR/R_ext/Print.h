/* miniR — R_ext/Print.h — Rprintf/REprintf */
#ifndef MINIR_R_EXT_PRINT_H
#define MINIR_R_EXT_PRINT_H

#include <stdarg.h>

#ifdef __cplusplus
extern "C" {
#endif

void Rprintf(const char *fmt, ...);
void REprintf(const char *fmt, ...);
void Rvprintf(const char *fmt, va_list ap);
void REvprintf(const char *fmt, va_list ap);

#ifdef __cplusplus
}
#endif

#endif
