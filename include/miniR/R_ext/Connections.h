/* miniR — R_ext/Connections.h — connection interface stubs */
#ifndef MINIR_R_EXT_CONNECTIONS_H
#define MINIR_R_EXT_CONNECTIONS_H

#include "../Rinternals.h"

/* Connection types — miniR doesn't expose the C connection interface,
   but some packages include this header. */
typedef struct Rconn *Rconnection;
struct Rconn {
    void *private_data;
};

SEXP R_new_custom_connection(const char *description, const char *mode,
                             const char *class_name, Rconnection *ptr);

#endif
