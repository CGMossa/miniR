/* miniR — R_ext/Memory.h — transient memory allocation */
#ifndef MINIR_R_EXT_MEMORY_H
#define MINIR_R_EXT_MEMORY_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stddef.h>

char *R_alloc(size_t nelem, int eltsize);

#ifndef MINIR_VMAXGET_DEFINED
#define MINIR_VMAXGET_DEFINED
static inline void *vmaxget(void) { return (void *)0; }
static inline void vmaxset(void *p) { (void)p; }
#endif


#ifdef __cplusplus
}
#endif
#endif
