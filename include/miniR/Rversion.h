/*
 * miniR — Rversion.h
 *
 * Compatibility header providing R version macros.
 * miniR reports as R 4.4.0 (modern — all conditional features enabled).
 */

#ifndef MINIR_RVERSION_H
#define MINIR_RVERSION_H

#define R_VERSION      0x40400  /* 4.4.0 */
#define R_MAJOR        "4"
#define R_MINOR        "4.0"
#define R_Version(v,p,s) (((v) * 65536) + ((p) * 256) + (s))

#endif /* MINIR_RVERSION_H */
