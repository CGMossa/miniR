/* miniR — R_ext/Parse.h — parse interface */
#ifndef MINIR_R_EXT_PARSE_H
#define MINIR_R_EXT_PARSE_H

#ifdef __cplusplus
extern "C" {
#endif

#include "../Rinternals.h"

typedef enum {
    PARSE_NULL, PARSE_OK, PARSE_INCOMPLETE, PARSE_ERROR, PARSE_EOF
} ParseStatus;

SEXP R_ParseVector(SEXP text, int n, ParseStatus *status, SEXP srcfile);


#ifdef __cplusplus
}
#endif
#endif
