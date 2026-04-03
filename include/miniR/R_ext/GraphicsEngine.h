/* miniR — R_ext/GraphicsEngine.h — graphics engine types */
#ifndef MINIR_R_EXT_GRAPHICSENGINE_H
#define MINIR_R_EXT_GRAPHICSENGINE_H

#include "../Rinternals.h"
#include "GraphicsDevice.h"

/* R graphics engine version */
#define R_GE_version 16

/* ── Unit types ── */
typedef enum {
    GE_DEVICE  = 0,
    GE_NDC     = 1,
    GE_INCHES  = 2,
    GE_CM      = 3,
    GE_PIXELS  = 4
} GEUnit;

/* ── Graphics context ── */
typedef struct {
    double col;       /* pen colour (lines, text) */
    double fill;      /* fill colour */
    double gamma;     /* gamma correction */
    double lwd;       /* line width */
    int lty;          /* line type */
    int lend;         /* line end */
    int ljoin;        /* line join */
    double lmitre;    /* line mitre limit */
    double cex;       /* character expansion */
    double ps;        /* point size */
    double lineheight;
    int fontface;     /* font face (1=plain, 2=bold, 3=italic, 4=bolditalic) */
    char fontfamily[201]; /* font family name */
    int patternFill;
} R_GE_gcontext;

typedef R_GE_gcontext *pGEcontext;

/* ── Device descriptor ── */
typedef struct _GEDevDesc {
    pDevDesc dev;
    Rboolean displayListOn;
    /* Additional fields exist in real R but are opaque to packages */
} GEDevDesc;

typedef GEDevDesc *pGEDevDesc;

/* ── Functions ── */
pGEDevDesc GEcurrentDevice(void);
/* cetype_t — character encoding (may already be defined in Rinternals.h) */
#ifndef cetype_t_is_defined
#define cetype_t_is_defined
typedef enum { CE_NATIVE = 0, CE_UTF8 = 1, CE_LATIN1 = 2, CE_BYTES = 3, CE_SYMBOL = 5, CE_ANY = 99 } cetype_t;
#endif

double GEStrWidth(const char *str, cetype_t enc, const pGEcontext gc, pGEDevDesc dd);
void GEStrMetric(const char *str, cetype_t enc, const pGEcontext gc, double *ascent,
                 double *descent, double *width, pGEDevDesc dd);
double GEfromDeviceWidth(double value, GEUnit to, pGEDevDesc dd);
double GEfromDeviceHeight(double value, GEUnit to, pGEDevDesc dd);
double GEtoDeviceWidth(double value, GEUnit from, pGEDevDesc dd);
double GEtoDeviceHeight(double value, GEUnit from, pGEDevDesc dd);
Rboolean GEdeviceDirty(pGEDevDesc dd);
void GEinitGraphics(pGEDevDesc dd);
pGEDevDesc GEgetDevice(int i);
int GEdeviceNumber(pGEDevDesc dd);
int curDevice(void);
int ndevNumber(pDevDesc dd);

/* Snapshot/restore */
SEXP GEcreateSnapshot(pGEDevDesc dd);
void GEplaySnapshot(SEXP snapshot, pGEDevDesc dd);

/* New page callback */
void GENewPage(const pGEcontext gc, pGEDevDesc dd);

#endif
