/* miniR -- R_ext/Complex.h -- Rcomplex type */
#ifndef MINIR_R_EXT_COMPLEX_H
#define MINIR_R_EXT_COMPLEX_H

/* Rcomplex -- define directly to avoid pulling in Rinternals.h
   (include order matters: some files include this before R_NO_REMAP is set) */
#ifndef Rcomplex_is_defined
#define Rcomplex_is_defined
typedef struct { double r; double i; } Rcomplex;
#endif

#endif /* MINIR_R_EXT_COMPLEX_H */
