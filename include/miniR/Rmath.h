/* miniR — Rmath.h — mathematical constants and functions */
#ifndef MINIR_RMATH_H
#define MINIR_RMATH_H

#include <math.h>
#include <float.h>
#include <limits.h>
#include "Rinternals.h"

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

/* R_pow / R_pow_di — power functions */
#define R_pow(x, y)  pow((x), (y))
#define R_pow_di(x, i) pow((x), (double)(i))

/* fmax2 / fmin2 — pairwise min/max */
static inline double fmax2(double x, double y) { return (x > y) ? x : y; }
static inline double fmin2(double x, double y) { return (x < y) ? x : y; }

/* imax2 / imin2 */
static inline int imax2(int x, int y) { return (x > y) ? x : y; }
static inline int imin2(int x, int y) { return (x < y) ? x : y; }

/* fsign — sign transfer */
static inline double fsign(double x, double y) { return (y >= 0) ? fabs(x) : -fabs(x); }

/* Distribution function stubs — return NaN for unimplemented */
/* Distribution function stubs — return NaN for unimplemented.
   Packages that need real stats should link against a math library. */
double dnorm(double x, double mu, double sigma, int log_p);
double pnorm(double x, double mu, double sigma, int lt, int lp);
double qnorm(double p, double mu, double sigma, int lt, int lp);
double qchisq(double p, double df, int lt, int lp);
double rexp(double scale);
double rnorm(double mu, double sigma);
double runif(double a, double b);
double rpois(double lambda);
double rbinom(double n, double p);
double choose(double n, double k);
double lchoose(double n, double k);
double lgammafn(double x);
double gammafn(double x);
double beta(double a, double b);
double lbeta(double a, double b);
double dbinom(double x, double n, double p, int lg);
double dpois(double x, double lambda, int lg);
double pgamma(double x, double shape, double scale, int lt, int lp);
double qgamma(double p, double shape, double scale, int lt, int lp);

/* Rboolean defined in Rinternals.h */

#endif /* MINIR_RMATH_H */
