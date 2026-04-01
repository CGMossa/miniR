/* miniR — R_ext/Connections.h — connection interface stubs */
#ifndef MINIR_R_EXT_CONNECTIONS_H
#define MINIR_R_EXT_CONNECTIONS_H

#include "../Rinternals.h"

#define R_CONNECTIONS_VERSION 1

/* Connection types — miniR stubs the C connection interface.
   Fields match GNU R's Rconn struct layout so packages compile. */
typedef struct Rconn *Rconnection;
/* In C++, 'class' and 'private' are keywords — remap them for C++ compilation */
#ifdef __cplusplus
#define class conn_class
#define private conn_private
#endif

struct Rconn {
    char *class;
    char *description;
    int enc; /* encoding */
    char mode[5];
    Rboolean isopen, incomplete, canread, canwrite, canseek, blocking, text;
    Rboolean isGzcon;
    void *private;
    /* Method pointers (stubs — miniR doesn't implement custom connections yet) */
    Rboolean (*open)(struct Rconn *);
    void (*close)(struct Rconn *);
    int (*vfprintf)(struct Rconn *, const char *, va_list);
    int (*fgetc)(struct Rconn *);
    int (*fgetc_internal)(struct Rconn *);
    double (*seek)(struct Rconn *, double, int, int);
    void (*truncate)(struct Rconn *);
    int (*fflush)(struct Rconn *);
    size_t (*read)(void *, size_t, size_t, struct Rconn *);
    size_t (*write)(const void *, size_t, size_t, struct Rconn *);
    void (*destroy)(struct Rconn *);
    /* Additional fields */
    int nPushBack;
    Rboolean UTF8out;
    void *id;
    void *ex_ptr;
    int status; /* for pipes etc */
};

#ifdef __cplusplus
#undef class
#undef private
#endif

SEXP R_new_custom_connection(const char *description, const char *mode,
                             const char *class_name, Rconnection *ptr);

/* Get a Rconnection from a SEXP connection object */
Rconnection R_GetConnection(SEXP con);

#endif
