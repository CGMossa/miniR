/* miniR — R_ext/BLAS.h — BLAS forward declarations */
#ifndef MINIR_R_EXT_BLAS_H
#define MINIR_R_EXT_BLAS_H

#include "RS.h" /* F77_NAME */

/* BLAS subroutines (void return) */
void F77_NAME(dgemm)();  void F77_NAME(dgemv)();
void F77_NAME(dsymm)();  void F77_NAME(dsymv)();
void F77_NAME(dtrmm)();  void F77_NAME(dtrsm)();
void F77_NAME(dtrsv)();  void F77_NAME(dscal)();
void F77_NAME(dcopy)();  void F77_NAME(daxpy)();
void F77_NAME(dswap)();  void F77_NAME(drotg)();
void F77_NAME(dsyrk)();  void F77_NAME(dspmv)();

/* BLAS functions (return double) */
double F77_NAME(ddot)();
double F77_NAME(dnrm2)();
double F77_NAME(dasum)();

/* BLAS functions (return int) */
int F77_NAME(idamax)();

/* Additional BLAS used by CRAN packages */
void F77_NAME(dger)();   void F77_NAME(dtpmv)();

#endif
