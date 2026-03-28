/* miniR — Rinterface.h — R interface stubs */
#ifndef MINIR_RINTERFACE_H
#define MINIR_RINTERFACE_H
#include "Rinternals.h"
extern int R_Interactive;
extern void (*R_CleanUp)(int, int, int);
#endif
