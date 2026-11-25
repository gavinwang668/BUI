/* BUI: OpenEXR's OpenEXRConfig.h (CMake generates it upstream from cmake/OpenEXRConfig.h.in), for
 * OpenEXRCore compiled into the kit: its functions are internal to the kit (not exported). */
#ifndef INCLUDED_OPENEXR_CONFIG_H
#define INCLUDED_OPENEXR_CONFIG_H 1

#define OPENEXR_VERSION_MAJOR 3
#define OPENEXR_VERSION_MINOR 5
#define OPENEXR_VERSION_PATCH 5
#define OPENEXR_VERSION_STRING "3.5.5"
#define OPENEXR_PACKAGE_STRING "OpenEXR 3.5.5"
#define OPENEXR_VERSION_RELEASE_TYPE ""
#define OPENEXR_VERSION_EXTRA ""
#define OPENEXR_LIB_VERSION_STRING "33.3.5.5"
#define OPENEXR_VERSION_HEX                                                                     \
  (((OPENEXR_VERSION_MAJOR) << 24) | ((OPENEXR_VERSION_MINOR) << 16) |                          \
   ((OPENEXR_VERSION_PATCH) << 8))

#define OPENEXR_CORE_FUNCTIONS_EMBEDDED
#if defined(__GNUC__) && !defined(_WIN32)
#  define OPENEXR_HIDDEN __attribute__((__visibility__("hidden")))
#else
#  define OPENEXR_HIDDEN
#endif
#define OPENEXR_EXPORT

#endif /* INCLUDED_OPENEXR_CONFIG_H */
