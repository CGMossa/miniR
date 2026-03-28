/* miniR — Rconfig.h — platform configuration for modern 64-bit systems */
#ifndef MINIR_RCONFIG_H
#define MINIR_RCONFIG_H

#include <stdint.h>

#define HAVE_UINTPTR_T 1

#if defined(__LP64__) || defined(_WIN64) || defined(__x86_64__) || defined(__aarch64__)
#  define SIZEOF_SIZE_T    8
#  define SIZEOF_LONG      8
#  define SIZEOF_DOUBLE    8
#  define SIZEOF_INT       4
#  define SIZEOF_LONG_DOUBLE 16
#  define LONG_VECTOR_SUPPORT 1
#  if defined(_WIN64)
#    undef SIZEOF_LONG
#    define SIZEOF_LONG    4
#  endif
#else
#  define SIZEOF_SIZE_T    4
#  define SIZEOF_LONG      4
#  define SIZEOF_DOUBLE    8
#  define SIZEOF_INT       4
#  define SIZEOF_LONG_DOUBLE 12
#endif

#define HAVE_LONG_DOUBLE  1
#define HAVE_ISNAN        1
#define HAVE_ISFINITE     1
#define HAVE_ALLOCA_H     1
#define HAVE_UNISTD_H     1
#define HAVE_STDINT_H     1
#define HAVE_INTTYPES_H   1
#define HAVE_EXPM1        1
#define HAVE_LOG1P        1
#define HAVE_HYPOT        1

#ifdef _WIN32
#  undef HAVE_ALLOCA_H
#  undef HAVE_UNISTD_H
#endif

#endif /* MINIR_RCONFIG_H */
