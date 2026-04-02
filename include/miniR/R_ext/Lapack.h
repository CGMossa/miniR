/* miniR — R_ext/Lapack.h — LAPACK/BLAS declarations for R packages.
 *
 * These routines are provided by the system LAPACK/BLAS library
 * (Accelerate framework on macOS, liblapack/libblas on Linux).
 * We don't declare argument types — the Fortran calling convention
 * is opaque from C. All declarations are variadic so any package's
 * call pattern compiles without prototype mismatch warnings.
 */
#ifndef MINIR_R_EXT_LAPACK_H
#define MINIR_R_EXT_LAPACK_H

#include "../R_ext/RS.h"  /* F77_NAME, F77_CALL */

/* LAPACK integer type */
#ifndef La_INT
#define La_INT int
#endif

/* Forward-declare all LAPACK/BLAS routines used by CRAN packages.
 * The actual symbols are resolved at link time against the system
 * LAPACK/BLAS (e.g. -framework Accelerate on macOS). */

/* Double-precision LAPACK */
void F77_NAME(dgebal)(...);  void F77_NAME(dgecon)(...);  void F77_NAME(dgees)(...);
void F77_NAME(dgels)(...);   void F77_NAME(dgemm)(...);   void F77_NAME(dgemv)(...);
void F77_NAME(dgeqrf)(...);  void F77_NAME(dgesdd)(...);  void F77_NAME(dgesvd)(...);
void F77_NAME(dgetrf)(...);  void F77_NAME(dgetri)(...);  void F77_NAME(dgetrs)(...);
void F77_NAME(dlacpy)(...);  void F77_NAME(dlange)(...);  void F77_NAME(dlansp)(...);
void F77_NAME(dlansy)(...);  void F77_NAME(dlantp)(...);  void F77_NAME(dlantr)(...);
void F77_NAME(dlarf)(...);   void F77_NAME(dlarfg)(...);  void F77_NAME(dlarfx)(...);
void F77_NAME(dormqr)(...);  void F77_NAME(dormtr)(...);
void F77_NAME(dpbtrf)(...);  void F77_NAME(dpocon)(...);  void F77_NAME(dposv)(...);
void F77_NAME(dpotrf)(...);  void F77_NAME(dpotri)(...);  void F77_NAME(dpotrs)(...);
void F77_NAME(dppcon)(...);  void F77_NAME(dpptrf)(...);  void F77_NAME(dpptri)(...);
void F77_NAME(dpptrs)(...);  void F77_NAME(dpstrf)(...);
void F77_NAME(dptsv)(...);   void F77_NAME(dpttrf)(...);
void F77_NAME(dscal)(...);   void F77_NAME(dswap)(...);   void F77_NAME(daxpy)(...);
void F77_NAME(dcopy)(...);   void F77_NAME(ddot)(...);    void F77_NAME(drotg)(...);
void F77_NAME(dspcon)(...);  void F77_NAME(dspmv)(...);   void F77_NAME(dsptrf)(...);
void F77_NAME(dsptri)(...);  void F77_NAME(dsptrs)(...);
void F77_NAME(dstedc)(...);  void F77_NAME(dsycon)(...);
void F77_NAME(dsyevd)(...);  void F77_NAME(dsyevr)(...);  void F77_NAME(dsyev)(...);
void F77_NAME(dsymm)(...);   void F77_NAME(dsymv)(...);   void F77_NAME(dsyrk)(...);
void F77_NAME(dsysv)(...);   void F77_NAME(dsytrd)(...);  void F77_NAME(dsytrf)(...);
void F77_NAME(dsytri)(...);  void F77_NAME(dsytrs)(...);
void F77_NAME(dtpcon)(...);  void F77_NAME(dtpmv)(...);   void F77_NAME(dtpsv)(...);
void F77_NAME(dtptri)(...);  void F77_NAME(dtptrs)(...);
void F77_NAME(dtrcon)(...);  void F77_NAME(dtrmm)(...);   void F77_NAME(dtrsm)(...);
void F77_NAME(dtrsv)(...);   void F77_NAME(dtrtri)(...);  void F77_NAME(dtrtrs)(...);
void F77_NAME(dgesv)(...);

/* Complex LAPACK */
void F77_NAME(zgecon)(...);  void F77_NAME(zgemm)(...);
void F77_NAME(zgetrf)(...);  void F77_NAME(zgetri)(...);  void F77_NAME(zgetrs)(...);
void F77_NAME(zlacpy)(...);  void F77_NAME(zlange)(...);  void F77_NAME(zlansp)(...);
void F77_NAME(zlansy)(...);  void F77_NAME(zlantp)(...);  void F77_NAME(zlantr)(...);
void F77_NAME(zpocon)(...);  void F77_NAME(zpotrf)(...);  void F77_NAME(zpotri)(...);
void F77_NAME(zpotrs)(...);  void F77_NAME(zppcon)(...);  void F77_NAME(zpptrf)(...);
void F77_NAME(zpptri)(...);  void F77_NAME(zpptrs)(...);  void F77_NAME(zpstrf)(...);
void F77_NAME(zspcon)(...);  void F77_NAME(zspmv)(...);   void F77_NAME(zsptrf)(...);
void F77_NAME(zsptri)(...);  void F77_NAME(zsptrs)(...);
void F77_NAME(zsycon)(...);  void F77_NAME(zsymm)(...);   void F77_NAME(zsymv)(...);
void F77_NAME(zsyrk)(...);   void F77_NAME(zsytrf)(...);  void F77_NAME(zsytri)(...);
void F77_NAME(zsytrs)(...);
void F77_NAME(ztpcon)(...);  void F77_NAME(ztpmv)(...);   void F77_NAME(ztptri)(...);
void F77_NAME(ztptrs)(...);  void F77_NAME(ztrcon)(...);  void F77_NAME(ztrmm)(...);
void F77_NAME(ztrtri)(...);  void F77_NAME(ztrtrs)(...);

#endif
