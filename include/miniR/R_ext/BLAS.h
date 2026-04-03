/* miniR — R_ext/BLAS.h — BLAS forward declarations */
#ifndef MINIR_R_EXT_BLAS_H
#define MINIR_R_EXT_BLAS_H

#ifdef __cplusplus
extern "C" {
#endif

#include "RS.h" /* F77_NAME */

/* BLAS subroutines (void return) */
void F77_NAME(dgemm)(FORTRAN_ARGS);  void F77_NAME(dgemv)(FORTRAN_ARGS);
void F77_NAME(dsymm)(FORTRAN_ARGS);  void F77_NAME(dsymv)(FORTRAN_ARGS);
void F77_NAME(dtrmm)(FORTRAN_ARGS);  void F77_NAME(dtrsm)(FORTRAN_ARGS);
void F77_NAME(dtrsv)(FORTRAN_ARGS);  void F77_NAME(dscal)(FORTRAN_ARGS);
void F77_NAME(dcopy)(FORTRAN_ARGS);  void F77_NAME(daxpy)(FORTRAN_ARGS);
void F77_NAME(dswap)(FORTRAN_ARGS);  void F77_NAME(drotg)(FORTRAN_ARGS);
void F77_NAME(dsyrk)(FORTRAN_ARGS);  void F77_NAME(dspmv)(FORTRAN_ARGS);

/* BLAS functions (return double) */
double F77_NAME(ddot)(FORTRAN_ARGS);
double F77_NAME(dnrm2)(FORTRAN_ARGS);
double F77_NAME(dasum)(FORTRAN_ARGS);

/* BLAS functions (return int) */
int F77_NAME(idamax)(FORTRAN_ARGS);

/* Additional BLAS used by CRAN packages */
void F77_NAME(dger)(FORTRAN_ARGS);   void F77_NAME(dtpmv)(FORTRAN_ARGS);


#ifdef __cplusplus
}
#endif
#endif
