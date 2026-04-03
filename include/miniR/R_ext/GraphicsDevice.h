/* miniR — R_ext/GraphicsDevice.h — graphics device types */
#ifndef MINIR_R_EXT_GRAPHICSDEVICE_H
#define MINIR_R_EXT_GRAPHICSDEVICE_H

#include "../Rinternals.h"

/* Forward declaration — full struct is opaque to most packages */
typedef struct _DevDesc DevDesc;
typedef DevDesc *pDevDesc;

/* Minimal DevDesc struct — packages that access fields directly need this */
struct _DevDesc {
    double left, right, bottom, top;  /* device region */
    double clipLeft, clipRight, clipBottom, clipTop;
    double xCharOffset, yCharOffset, yLineBias;
    double ipr[2];    /* inches per raster unit */
    double cra[2];    /* character size in rasters */
    double gamma;
    Rboolean canClip;
    Rboolean canChangeGamma;
    int canHAdj;      /* text horizontal adjustment */
    double startps;   /* initial point size */
    int startcol, startfill, startlty;
    double startfont;
    double startgamma;
    void *deviceSpecific;  /* package-specific data */
    Rboolean displayListOn;
    Rboolean canGenMouseDown, canGenMouseMove, canGenMouseUp, canGenKeybd, canGenIdle;
    Rboolean hasTextUTF8, wantSymbolUTF8;
    Rboolean useRotatedTextInContour;
    int haveTransparency, haveTransparentBg, haveRaster, haveCapture, haveLocator;
};

#endif
