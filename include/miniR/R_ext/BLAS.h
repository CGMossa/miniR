/* miniR — R_ext/BLAS.h — BLAS forward declarations */
#ifndef MINIR_R_EXT_BLAS_H
#define MINIR_R_EXT_BLAS_H

#include "RS.h" /* F77_NAME */

/* Common BLAS routines — all variadic since they're Fortran */
void F77_NAME(dgemm)(...);  void F77_NAME(dgemv)(...);
void F77_NAME(dsymm)(...);  void F77_NAME(dsymv)(...);
void F77_NAME(dtrmm)(...);  void F77_NAME(dtrsm)(...);
void F77_NAME(dtrsv)(...);  void F77_NAME(dscal)(...);
void F77_NAME(dcopy)(...);  void F77_NAME(daxpy)(...);
void F77_NAME(ddot)(...);   void F77_NAME(dswap)(...);
void F77_NAME(drotg)(...);  void F77_NAME(dsyrk)(...);
void F77_NAME(dnrm2)(...);  void F77_NAME(dasum)(...);
void F77_NAME(idamax)(...);

#endif
