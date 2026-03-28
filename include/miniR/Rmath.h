/* miniR -- Rmath.h -- mathematical constants and distribution function stubs */
#ifndef MINIR_RMATH_H
#define MINIR_RMATH_H

#include <math.h>
#include <float.h>
#include <limits.h>
#include "Rinternals.h"
#include "Rconfig.h"

/* Mathematical constants */
#ifndef M_PI
#define M_PI        3.141592653589793238462643383280
#endif
#ifndef M_E
#define M_E         2.718281828459045235360287471353
#endif
#ifndef M_LOG2E
#define M_LOG2E     1.442695040888963407359924681002
#endif
#ifndef M_LN2
#define M_LN2       0.693147180559945309417232121458
#endif
#ifndef M_LN10
#define M_LN10      2.302585092994045684017991454684
#endif
#ifndef M_SQRT2
#define M_SQRT2     1.414213562373095048801688724210
#endif
#ifndef M_1_SQRT_2PI
#define M_1_SQRT_2PI 0.398942280401432677939946059934
#endif
#ifndef M_SQRT_2dPI
#define M_SQRT_2dPI  0.797884560802865355879892119869
#endif
#ifndef M_LOG10_2
#define M_LOG10_2   0.301029995663981195213738894947
#endif
#ifndef M_2PI
#define M_2PI       6.283185307179586476925286766559
#endif
#ifndef M_SQRT_PI
#define M_SQRT_PI   1.772453850905516027298167483341
#endif
#ifndef M_1_PI
#define M_1_PI      0.318309886183790671537767526745
#endif
#ifndef M_SQRT_32
#define M_SQRT_32   5.656854249492380195206754896838
#endif
#ifndef M_LN_SQRT_2PI
#define M_LN_SQRT_2PI  0.918938533204672741780329736406  /* log(sqrt(2*pi)) */
#endif
#ifndef M_LN_SQRT_PId2
#define M_LN_SQRT_PId2 0.225791352644727432363097614947  /* log(sqrt(pi/2)) */
#endif

#ifdef __cplusplus
extern "C" {
#endif

/* R_pow / R_pow_di -- power functions */
#define R_pow(x, y)  pow((x), (y))
#define R_pow_di(x, i) pow((x), (double)(i))

/* fmax2 / fmin2 -- pairwise min/max */
static inline double fmax2(double x, double y) { return (x > y) ? x : y; }
static inline double fmin2(double x, double y) { return (x < y) ? x : y; }

/* imax2 / imin2 */
static inline int imax2(int x, int y) { return (x > y) ? x : y; }
static inline int imin2(int x, int y) { return (x < y) ? x : y; }

/* fsign -- sign transfer */
static inline double fsign(double x, double y) { return (y >= 0) ? fabs(x) : -fabs(x); }

/* Rf_ prefixed aliases for imax2/imin2/fmax2/fmin2/fsign */
#define Rf_imax2 imax2
#define Rf_imin2 imin2
#define Rf_fmax2 fmax2
#define Rf_fmin2 fmin2
#define Rf_fsign fsign

/* Rounding and truncation */
double Rf_fround(double x, double digits);
double Rf_ftrunc(double x);
double Rf_fprec(double x, double digits);
double Rf_sign(double x);

/* Gamma and related functions */
double Rf_gammafn(double x);
double Rf_lgammafn(double x);
double Rf_lgammafn_sign(double x, int *sgn);
double Rf_digamma(double x);
double Rf_trigamma(double x);
double Rf_tetragamma(double x);
double Rf_pentagamma(double x);
double Rf_psigamma(double x, double deriv);
void   Rf_dpsifn(double x, int n, int kode, int m, double *ans, int *nz, int *ierr);
double Rf_beta(double a, double b);
double Rf_lbeta(double a, double b);
double Rf_choose(double n, double k);
double Rf_lchoose(double n, double k);

/* Log-space arithmetic */
double Rf_log1pmx(double x);
double log1pexp(double x);
double Rf_lgamma1p(double a);
double Rf_logspace_add(double lx, double ly);
double Rf_logspace_sub(double lx, double ly);

/* Normal Distribution */
double Rf_dnorm4(double x, double mu, double sigma, int lg);
double Rf_pnorm5(double x, double mu, double sigma, int lt, int lg);
double Rf_qnorm5(double p, double mu, double sigma, int lt, int lg);
double Rf_rnorm(double mu, double sigma);
void   Rf_pnorm_both(double x, double *cum, double *ccum, int lt, int lg);

/* Convenience aliases for normal distribution */
#define dnorm4 Rf_dnorm4
#define pnorm5 Rf_pnorm5
#define qnorm5 Rf_qnorm5
#define rnorm  Rf_rnorm
#define pnorm_both Rf_pnorm_both

/* Uniform Distribution */
double Rf_dunif(double x, double a, double b, int lg);
double Rf_punif(double x, double a, double b, int lt, int lg);
double Rf_qunif(double p, double a, double b, int lt, int lg);
double Rf_runif(double a, double b);

/* Gamma Distribution */
double Rf_dgamma(double x, double shp, double scl, int lg);
double Rf_pgamma(double x, double alp, double scl, int lt, int lg);
double Rf_qgamma(double p, double alp, double scl, int lt, int lg);
double Rf_rgamma(double a, double scl);

/* Beta Distribution */
double Rf_dbeta(double x, double a, double b, int lg);
double Rf_pbeta(double x, double p, double q, int lt, int lg);
double Rf_qbeta(double a, double p, double q, int lt, int lg);
double Rf_rbeta(double a, double b);

/* Lognormal Distribution */
double Rf_dlnorm(double x, double ml, double sl, int lg);
double Rf_plnorm(double x, double ml, double sl, int lt, int lg);
double Rf_qlnorm(double p, double ml, double sl, int lt, int lg);
double Rf_rlnorm(double ml, double sl);

/* Chi-squared Distribution */
double Rf_dchisq(double x, double df, int lg);
double Rf_pchisq(double x, double df, int lt, int lg);
double Rf_qchisq(double p, double df, int lt, int lg);
double Rf_rchisq(double df);

/* Non-central Chi-squared Distribution */
double Rf_dnchisq(double x, double df, double ncp, int lg);
double Rf_pnchisq(double x, double df, double ncp, int lt, int lg);
double Rf_qnchisq(double p, double df, double ncp, int lt, int lg);
double Rf_rnchisq(double df, double lb);

/* F Distribution */
double Rf_df(double x, double df1, double df2, int lg);
double Rf_pf(double x, double df1, double df2, int lt, int lg);
double Rf_qf(double p, double df1, double df2, int lt, int lg);
double Rf_rf(double df1, double df2);

/* Student t Distribution */
double Rf_dt(double x, double n, int lg);
double Rf_pt(double x, double n, int lt, int lg);
double Rf_qt(double p, double n, int lt, int lg);
double Rf_rt(double n);

/* Binomial Distribution */
double Rf_dbinom(double x, double n, double p, int lg);
double Rf_pbinom(double x, double n, double p, int lt, int lg);
double Rf_qbinom(double p, double n, double m, int lt, int lg);
double Rf_rbinom(double n, double p);
void   rmultinom(int n, double *prob, int k, int *rn);

/* Cauchy Distribution */
double Rf_dcauchy(double x, double lc, double sl, int lg);
double Rf_pcauchy(double x, double lc, double sl, int lt, int lg);
double Rf_qcauchy(double p, double lc, double sl, int lt, int lg);
double Rf_rcauchy(double lc, double sl);

/* Exponential Distribution */
double Rf_dexp(double x, double sl, int lg);
double Rf_pexp(double x, double sl, int lt, int lg);
double Rf_qexp(double p, double sl, int lt, int lg);
double Rf_rexp(double sl);

/* Geometric Distribution */
double Rf_dgeom(double x, double p, int lg);
double Rf_pgeom(double x, double p, int lt, int lg);
double Rf_qgeom(double p, double pb, int lt, int lg);
double Rf_rgeom(double p);

/* Hypergeometric Distribution */
double Rf_dhyper(double x, double r, double b, double n, int lg);
double Rf_phyper(double x, double r, double b, double n, int lt, int lg);
double Rf_qhyper(double p, double r, double b, double n, int lt, int lg);
double Rf_rhyper(double r, double b, double n);

/* Negative Binomial Distribution */
double Rf_dnbinom(double x, double sz, double pb, int lg);
double Rf_pnbinom(double x, double sz, double pb, int lt, int lg);
double Rf_qnbinom(double p, double sz, double pb, int lt, int lg);
double Rf_rnbinom(double sz, double pb);
double Rf_dnbinom_mu(double x, double sz, double mu, int lg);
double Rf_pnbinom_mu(double x, double sz, double mu, int lt, int lg);
double Rf_qnbinom_mu(double x, double sz, double mu, int lt, int lg);

/* Non-prefixed aliases (Rcpp uses these after undoRmath.h removes macros) */
#define dnbinom_mu Rf_dnbinom_mu
#define pnbinom_mu Rf_pnbinom_mu
#define qnbinom_mu Rf_qnbinom_mu

/* Poisson Distribution */
double Rf_dpois(double x, double lb, int lg);
double Rf_ppois(double x, double lb, int lt, int lg);
double Rf_qpois(double p, double lb, int lt, int lg);
double Rf_rpois(double mu);

/* Weibull Distribution */
double Rf_dweibull(double x, double sh, double sl, int lg);
double Rf_pweibull(double x, double sh, double sl, int lt, int lg);
double Rf_qweibull(double p, double sh, double sl, int lt, int lg);
double Rf_rweibull(double sh, double sl);

/* Logistic Distribution */
double Rf_dlogis(double x, double lc, double sl, int lg);
double Rf_plogis(double x, double lc, double sl, int lt, int lg);
double Rf_qlogis(double p, double lc, double sl, int lt, int lg);
double Rf_rlogis(double lc, double sl);

/* Non-central Beta Distribution */
double Rf_dnbeta(double x, double a, double b, double ncp, int lg);
double Rf_pnbeta(double x, double a, double b, double ncp, int lt, int lg);
double Rf_qnbeta(double p, double a, double b, double ncp, int lt, int lg);

/* Non-central F Distribution */
double Rf_dnf(double x, double df1, double df2, double ncp, int lg);
double Rf_pnf(double x, double df1, double df2, double ncp, int lt, int lg);
double Rf_qnf(double p, double df1, double df2, double ncp, int lt, int lg);

/* Non-central Student t Distribution */
double Rf_dnt(double x, double df, double ncp, int lg);
double Rf_pnt(double x, double df, double ncp, int lt, int lg);
double Rf_qnt(double p, double df, double ncp, int lt, int lg);

/* Studentized Range Distribution */
double Rf_ptukey(double q, double rr, double cc, double df, int lt, int lg);
double Rf_qtukey(double p, double rr, double cc, double df, int lt, int lg);

/* Wilcoxon Rank Sum Distribution */
double Rf_dwilcox(double x, double m, double n, int lg);
double Rf_pwilcox(double q, double m, double n, int lt, int lg);
double Rf_qwilcox(double x, double m, double n, int lt, int lg);
double Rf_rwilcox(double m, double n);

/* Wilcoxon Signed Rank Distribution */
double Rf_dsignrank(double x, double n, int lg);
double Rf_psignrank(double x, double n, int lt, int lg);
double Rf_qsignrank(double x, double n, int lt, int lg);
double Rf_rsignrank(double n);

/* Bessel Functions */
double Rf_bessel_i(double x, double al, double ex);
double Rf_bessel_j(double x, double al);
double Rf_bessel_k(double x, double al, double ex);
double Rf_bessel_y(double x, double al);
double Rf_bessel_i_ex(double x, double al, double ex, double *bi);
double Rf_bessel_j_ex(double x, double al, double *bj);
double Rf_bessel_k_ex(double x, double al, double ex, double *bk);
double Rf_bessel_y_ex(double x, double al, double *by);

/* Hypot */
double Rf_hypot(double a, double b);

#ifdef __cplusplus
}
#endif

#endif /* MINIR_RMATH_H */
