/* miniR -- R_ext/Boolean.h -- Rboolean type and TRUE/FALSE */
#ifndef MINIR_R_EXT_BOOLEAN_H
#define MINIR_R_EXT_BOOLEAN_H

#ifdef __cplusplus
extern "C" {
#endif

/* Rboolean -- define directly to avoid pulling in Rinternals.h
   (include order matters: some files include this before R_NO_REMAP is set) */
#ifndef Rboolean_is_defined
#define Rboolean_is_defined
typedef enum { FALSE = 0, TRUE = 1 } Rboolean;
#endif


#ifdef __cplusplus
}
#endif
#endif /* MINIR_R_EXT_BOOLEAN_H */
