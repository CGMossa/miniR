/* miniR — R_ext/Applic.h — optimization routines (stubs) */
#ifndef MINIR_R_EXT_APPLIC_H
#define MINIR_R_EXT_APPLIC_H

typedef void (*optimfn)(int, double *, double *, void *);
typedef void (*optimgr)(int, double *, double *, void *);

void nmmin(int n, double *xin, double *x, double *Fmin, optimfn fn,
           int *fail, double abstol, double intol, void *ex,
           double alpha, double beta, double gamma, int trace,
           int *fncount, int maxit);
void vmmin(int n, double *x, double *Fmin, optimfn fn, optimgr gr,
           int maxit, int trace, int *mask, double abstol, double reltol,
           int nREPORT, void *ex, int *fncount, int *grcount, int *fail);
void cgmin(int n, double *xin, double *x, double *Fmin, optimfn fn,
           optimgr gr, int *fail, double abstol, double intol, void *ex,
           int type, int trace, int *fncount, int *grcount, int maxit);
void lbfgsb(int n, int m, double *x, double *l, double *u, int *nbd,
            double *Fmin, optimfn fn, optimgr gr, int *fail, void *ex,
            double factr, double pgtol, int *fncount, int *grcount,
            int maxit, char *msg, int trace, int nREPORT);

#endif
