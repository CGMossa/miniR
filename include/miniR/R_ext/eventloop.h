/* miniR — R_ext/eventloop.h — event loop stubs */
#ifndef MINIR_R_EXT_EVENTLOOP_H
#define MINIR_R_EXT_EVENTLOOP_H

#include "../Rinternals.h"

/* Input handler types for R's event loop.
   miniR doesn't implement the R event loop, but packages that
   hook into it need these declarations to compile. */

typedef void (*fd_set_check_func)(void *);
typedef void (*fd_set_action_func)(void *);

typedef struct _InputHandler {
    int fileDescriptor;
    fd_set_check_func check;
    fd_set_action_func action;
    void *userData;
    struct _InputHandler *next;
    int active;
} InputHandler;

#ifdef __cplusplus
extern "C" {
#endif

InputHandler *addInputHandler(InputHandler *handlers, int fd,
    fd_set_action_func action, int activity);
InputHandler *removeInputHandler(InputHandler **handlers, InputHandler *handler);
InputHandler *getInputHandler(InputHandler *handlers, int fd);

#ifdef __cplusplus
}
#endif

/* Global input handler chain */
extern InputHandler *R_InputHandlers;

/* R_PolledEvents — called during event loop idle */
extern void (*R_PolledEvents)(void);
extern int R_wait_usec;

#endif
