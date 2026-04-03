/* miniR — R_ext/Linpack.h — LAPACK/BLAS Fortran routine declarations */
#ifndef MINIR_R_EXT_LINPACK_H
#define MINIR_R_EXT_LINPACK_H

#ifdef __cplusplus
extern "C" {
#endif

/* F77_NAME / F77_CALL / F77_SUB — Fortran name mangling */
#ifndef F77_NAME
#define F77_NAME(x) x ## _
#endif
#ifndef F77_CALL
#define F77_CALL(x) x ## _
#endif
#ifndef F77_SUB
#define F77_SUB(x) x ## _
#endif

/* LAPACK/LINPACK routines */
void dqrdc2_(double *x, int *ldx, int *n, int *p, double *tol,
             int *k, double *qraux, int *jpvt, double *work);
void dqrsl_(double *x, int *ldx, int *n, int *k, double *qraux,
            double *y, double *qy, double *qty, double *b, double *rsd,
            double *xb, int *job, int *info);
void dgemm_(const char *transa, const char *transb, int *m, int *n,
            int *k, double *alpha, double *a, int *lda, double *b,
            int *ldb, double *beta, double *c, int *ldc);
void dsyrk_(const char *uplo, const char *trans, int *n, int *k,
            double *alpha, double *a, int *lda, double *beta,
            double *c, int *ldc);
void dgemv_(const char *trans, int *m, int *n, double *alpha,
            double *a, int *lda, double *x, int *incx, double *beta,
            double *y, int *incy);
void dpotrf_(const char *uplo, int *n, double *a, int *lda, int *info);
void dpotri_(const char *uplo, int *n, double *a, int *lda, int *info);
void dtrsm_(const char *side, const char *uplo, const char *transa,
            const char *diag, int *m, int *n, double *alpha, double *a,
            int *lda, double *b, int *ldb);
void dtrsl_(double *t, int *ldt, int *n, double *b, int *job, int *info);
void chol_(double *a, int *lda, int *n, double *v, int *info);
void rs_(int *nm, int *n, double *a, double *w, int *matz,
         double *z, double *fv1, double *fv2, int *ierr);

/* Memcpy / Memzero */
#ifndef Memcpy
#define Memcpy(to, from, n) memcpy((to), (from), (size_t)(n) * sizeof(*(to)))
#define Memzero(to, n)      memset((to), 0, (size_t)(n) * sizeof(*(to)))
#endif


#ifdef __cplusplus
}
#endif
#endif
