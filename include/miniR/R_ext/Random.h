/* miniR — R_ext/Random.h — RNG functions */
#ifndef MINIR_R_EXT_RANDOM_H
#define MINIR_R_EXT_RANDOM_H

void GetRNGstate(void);
void PutRNGstate(void);
double unif_rand(void);
double norm_rand(void);
double exp_rand(void);

typedef enum {
    WICHMANN_HILL, MARSAGLIA_MULTICARRY, SUPER_DUPER,
    MERSENNE_TWISTER, KNUTH_TAOCP, USER_UNIF,
    KNUTH_TAOCP2, LECUYER_CMRG, DEFAULT_RNG
} RNGtype;

#endif
