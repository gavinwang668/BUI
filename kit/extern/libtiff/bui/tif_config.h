/* BUI: libtiff's tif_config.h (CMake generates it upstream from tif_config.h.cmake.in). */
#include "tiffconf.h"

#define CHUNKY_STRIP_READ_SUPPORT 1
#define HAVE_ASSERT_H 1
#define HAVE_FCNTL_H 1
#ifndef _WIN32
#  define HAVE_UNISTD_H 1
#  define HAVE_SYS_TYPES_H 1
#  define HAVE_STRINGS_H 1
#endif
#define PACKAGE "LibTIFF Software"
#define PACKAGE_BUGREPORT "tiff@lists.osgeo.org"
#define PACKAGE_NAME "LibTIFF Software"
#define PACKAGE_TARNAME "tiff"
#define PACKAGE_URL ""
#if defined(_WIN64) || defined(__LP64__) || (defined(__SIZEOF_POINTER__) && __SIZEOF_POINTER__ == 8)
#  define SIZEOF_SIZE_T 8
#else
#  define SIZEOF_SIZE_T 4
#endif
#define STRIP_SIZE_DEFAULT 8192
#define TIFF_MAX_DIR_COUNT 1048576
#if defined(__BYTE_ORDER__) && __BYTE_ORDER__ == __ORDER_BIG_ENDIAN__
#  define WORDS_BIGENDIAN 1
#else
#  define WORDS_BIGENDIAN 0
#endif

#if !defined(__MINGW32__)
#  define TIFF_SIZE_FORMAT "zu"
#endif
#if SIZEOF_SIZE_T == 8
#  define TIFF_SSIZE_FORMAT PRId64
#  if defined(__MINGW32__)
#    define TIFF_SIZE_FORMAT PRIu64
#  endif
#else
#  define TIFF_SSIZE_FORMAT PRId32
#  if defined(__MINGW32__)
#    define TIFF_SIZE_FORMAT PRIu32
#  endif
#endif
