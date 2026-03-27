/* miniR — R_ext/Visibility.h — symbol visibility macros */
#ifndef MINIR_R_EXT_VISIBILITY_H
#define MINIR_R_EXT_VISIBILITY_H

#define attribute_visible  __attribute__((visibility("default")))
#define attribute_hidden   __attribute__((visibility("hidden")))

#endif
