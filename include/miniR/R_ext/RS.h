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

#endif
