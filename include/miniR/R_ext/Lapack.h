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

#ifdef __cplusplus
extern "C" {
#endif

#include "../R_ext/RS.h"


/* LAPACK integer type */
#ifndef La_INT
#define La_INT int
#endif

/* Forward-declare all LAPACK/BLAS routines used by CRAN packages.
 * The actual symbols are resolved at link time against the system
 * LAPACK/BLAS (e.g. -framework Accelerate on macOS). */

/* Double-precision LAPACK */
void F77_NAME(dgebal)(FORTRAN_ARGS);  void F77_NAME(dgecon)(FORTRAN_ARGS);  void F77_NAME(dgees)(FORTRAN_ARGS);
void F77_NAME(dgels)(FORTRAN_ARGS);   void F77_NAME(dgemm)(FORTRAN_ARGS);   void F77_NAME(dgemv)(FORTRAN_ARGS);
void F77_NAME(dgeqrf)(FORTRAN_ARGS);  void F77_NAME(dgesdd)(FORTRAN_ARGS);  void F77_NAME(dgesvd)(FORTRAN_ARGS);
void F77_NAME(dgetrf)(FORTRAN_ARGS);  void F77_NAME(dgetri)(FORTRAN_ARGS);  void F77_NAME(dgetrs)(FORTRAN_ARGS);
void F77_NAME(dlacpy)(FORTRAN_ARGS);  double F77_NAME(dlange)(FORTRAN_ARGS);  double F77_NAME(dlansp)(FORTRAN_ARGS);
double F77_NAME(dlansy)(FORTRAN_ARGS);  double F77_NAME(dlantp)(FORTRAN_ARGS);  double F77_NAME(dlantr)(FORTRAN_ARGS);
void F77_NAME(dlarf)(FORTRAN_ARGS);   void F77_NAME(dlarfg)(FORTRAN_ARGS);  void F77_NAME(dlarfx)(FORTRAN_ARGS);
void F77_NAME(dormqr)(FORTRAN_ARGS);  void F77_NAME(dormtr)(FORTRAN_ARGS);
void F77_NAME(dpbtrf)(FORTRAN_ARGS);  void F77_NAME(dpocon)(FORTRAN_ARGS);  void F77_NAME(dposv)(FORTRAN_ARGS);
void F77_NAME(dpotrf)(FORTRAN_ARGS);  void F77_NAME(dpotri)(FORTRAN_ARGS);  void F77_NAME(dpotrs)(FORTRAN_ARGS);
void F77_NAME(dppcon)(FORTRAN_ARGS);  void F77_NAME(dpptrf)(FORTRAN_ARGS);  void F77_NAME(dpptri)(FORTRAN_ARGS);
void F77_NAME(dpptrs)(FORTRAN_ARGS);  void F77_NAME(dpstrf)(FORTRAN_ARGS);
void F77_NAME(dptsv)(FORTRAN_ARGS);   void F77_NAME(dpttrf)(FORTRAN_ARGS);
void F77_NAME(dscal)(FORTRAN_ARGS);   void F77_NAME(dswap)(FORTRAN_ARGS);   void F77_NAME(daxpy)(FORTRAN_ARGS);
void F77_NAME(dcopy)(FORTRAN_ARGS);   double F77_NAME(ddot)(FORTRAN_ARGS);    void F77_NAME(drotg)(FORTRAN_ARGS);
void F77_NAME(dspcon)(FORTRAN_ARGS);  void F77_NAME(dspmv)(FORTRAN_ARGS);   void F77_NAME(dsptrf)(FORTRAN_ARGS);
void F77_NAME(dsptri)(FORTRAN_ARGS);  void F77_NAME(dsptrs)(FORTRAN_ARGS);
void F77_NAME(dstedc)(FORTRAN_ARGS);  void F77_NAME(dsycon)(FORTRAN_ARGS);
void F77_NAME(dsyevd)(FORTRAN_ARGS);  void F77_NAME(dsyevr)(FORTRAN_ARGS);  void F77_NAME(dsyev)(FORTRAN_ARGS);
void F77_NAME(dsymm)(FORTRAN_ARGS);   void F77_NAME(dsymv)(FORTRAN_ARGS);   void F77_NAME(dsyrk)(FORTRAN_ARGS);
void F77_NAME(dsysv)(FORTRAN_ARGS);   void F77_NAME(dsytrd)(FORTRAN_ARGS);  void F77_NAME(dsytrf)(FORTRAN_ARGS);
void F77_NAME(dsytri)(FORTRAN_ARGS);  void F77_NAME(dsytrs)(FORTRAN_ARGS);
void F77_NAME(dtpcon)(FORTRAN_ARGS);  void F77_NAME(dtpmv)(FORTRAN_ARGS);   void F77_NAME(dtpsv)(FORTRAN_ARGS);
void F77_NAME(dtptri)(FORTRAN_ARGS);  void F77_NAME(dtptrs)(FORTRAN_ARGS);
void F77_NAME(dtrcon)(FORTRAN_ARGS);  void F77_NAME(dtrmm)(FORTRAN_ARGS);   void F77_NAME(dtrsm)(FORTRAN_ARGS);
void F77_NAME(dtrsv)(FORTRAN_ARGS);   void F77_NAME(dtrtri)(FORTRAN_ARGS);  void F77_NAME(dtrtrs)(FORTRAN_ARGS);
void F77_NAME(dgesv)(FORTRAN_ARGS);

/* Complex LAPACK */
void F77_NAME(zgecon)(FORTRAN_ARGS);  void F77_NAME(zgemm)(FORTRAN_ARGS);
void F77_NAME(zgetrf)(FORTRAN_ARGS);  void F77_NAME(zgetri)(FORTRAN_ARGS);  void F77_NAME(zgetrs)(FORTRAN_ARGS);
void F77_NAME(zlacpy)(FORTRAN_ARGS);  double F77_NAME(zlange)(FORTRAN_ARGS);  double F77_NAME(zlansp)(FORTRAN_ARGS);
double F77_NAME(zlansy)(FORTRAN_ARGS);  double F77_NAME(zlantp)(FORTRAN_ARGS);  double F77_NAME(zlantr)(FORTRAN_ARGS);
void F77_NAME(zpocon)(FORTRAN_ARGS);  void F77_NAME(zpotrf)(FORTRAN_ARGS);  void F77_NAME(zpotri)(FORTRAN_ARGS);
void F77_NAME(zpotrs)(FORTRAN_ARGS);  void F77_NAME(zppcon)(FORTRAN_ARGS);  void F77_NAME(zpptrf)(FORTRAN_ARGS);
void F77_NAME(zpptri)(FORTRAN_ARGS);  void F77_NAME(zpptrs)(FORTRAN_ARGS);  void F77_NAME(zpstrf)(FORTRAN_ARGS);
void F77_NAME(zspcon)(FORTRAN_ARGS);  void F77_NAME(zspmv)(FORTRAN_ARGS);   void F77_NAME(zsptrf)(FORTRAN_ARGS);
void F77_NAME(zsptri)(FORTRAN_ARGS);  void F77_NAME(zsptrs)(FORTRAN_ARGS);
void F77_NAME(zsycon)(FORTRAN_ARGS);  void F77_NAME(zsymm)(FORTRAN_ARGS);   void F77_NAME(zsymv)(FORTRAN_ARGS);
void F77_NAME(zsyrk)(FORTRAN_ARGS);   void F77_NAME(zsytrf)(FORTRAN_ARGS);  void F77_NAME(zsytri)(FORTRAN_ARGS);
void F77_NAME(zsytrs)(FORTRAN_ARGS);
void F77_NAME(ztpcon)(FORTRAN_ARGS);  void F77_NAME(ztpmv)(FORTRAN_ARGS);   void F77_NAME(ztptri)(FORTRAN_ARGS);
void F77_NAME(ztptrs)(FORTRAN_ARGS);  void F77_NAME(ztrcon)(FORTRAN_ARGS);  void F77_NAME(ztrmm)(FORTRAN_ARGS);
void F77_NAME(ztrtri)(FORTRAN_ARGS);  void F77_NAME(ztrtrs)(FORTRAN_ARGS);


#ifdef __cplusplus
}
#endif
#endif
