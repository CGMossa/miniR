/*
 * miniR — Rdefines.h
 *
 * Compatibility header — some older packages include this.
 * It provides alternative names for common R API functions.
 */

#ifndef MINIR_RDEFINES_H
#define MINIR_RDEFINES_H

#include "Rinternals.h"

#define NEW_NUMERIC(n)    Rf_allocVector(REALSXP, (n))
#define NEW_INTEGER(n)    Rf_allocVector(INTSXP, (n))
#define NEW_LOGICAL(n)    Rf_allocVector(LGLSXP, (n))
#define NEW_CHARACTER(n)  Rf_allocVector(STRSXP, (n))
#define NEW_LIST(n)       Rf_allocVector(VECSXP, (n))
#define NEW_RAW(n)        Rf_allocVector(RAWSXP, (n))

#define NUMERIC_POINTER(x)    REAL(x)
#define INTEGER_POINTER(x)    INTEGER(x)
#define LOGICAL_POINTER(x)    LOGICAL(x)

#define GET_LENGTH(x)     LENGTH(x)
#define SET_LENGTH(x, n)  ((x)->length = (int32_t)(n))

#define GET_NAMES(x)      Rf_getAttrib((x), R_NamesSymbol)
#define SET_NAMES(x, v)   Rf_setAttrib((x), R_NamesSymbol, (v))

#define GET_CLASS(x)      Rf_getAttrib((x), R_ClassSymbol)
#define SET_CLASS(x, v)   Rf_setAttrib((x), R_ClassSymbol, (v))

#define COPY_TO_USER_STRING(x) ((const char*)R_CHAR(x))
#define AS_NUMERIC(x)     Rf_coerceVector((x), REALSXP)
#define AS_INTEGER(x)     Rf_coerceVector((x), INTSXP)
#define AS_LOGICAL(x)     Rf_coerceVector((x), LGLSXP)
#define AS_CHARACTER(x)   Rf_coerceVector((x), STRSXP)

#endif /* MINIR_RDEFINES_H */
