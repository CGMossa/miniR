/* miniR — R_ext/RS.h — R_Calloc/R_Free/R_Realloc macros */
#ifndef MINIR_R_EXT_RS_H
#define MINIR_R_EXT_RS_H

#include <stdlib.h>

#define R_Calloc(n, t)     ((t*)calloc((size_t)(n), sizeof(t)))
#define R_Free(p)          (free((void*)(p)), (p) = NULL)
#define R_Realloc(p, n, t) ((t*)realloc((void*)(p), (size_t)(n) * sizeof(t)))

#define Calloc(n, t)       R_Calloc(n, t)
#define Free(p)            R_Free(p)
#define Realloc(p, n, t)   R_Realloc(p, n, t)

/* Fortran name mangling (same as R.h) */
#ifndef F77_NAME
#define F77_NAME(x) x ## _
#endif
#ifndef F77_CALL
#define F77_CALL(x) x ## _
#endif
#ifndef F77_SUB
#define F77_SUB(x)  x ## _
#endif

/* String length type for Fortran */
#ifndef FCONE
#define FCONE
#endif

#endif
