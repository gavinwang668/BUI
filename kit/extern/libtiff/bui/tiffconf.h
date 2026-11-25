/* BUI: libtiff's tiffconf.h (CMake generates it upstream from tiffconf.h.cmake.in), for the
 * decoder subset kit/CMakeLists.txt builds: the codecs listed in ../README.bui. */
#ifndef _TIFFCONF_
#define _TIFFCONF_

#include <inttypes.h>
#include <stddef.h>
#include <stdint.h>

#define TIFF_INT16_T int16_t
#define TIFF_INT32_T int32_t
#define TIFF_INT64_T int64_t
#define TIFF_INT8_T int8_t
#define TIFF_UINT16_T uint16_t
#define TIFF_UINT32_T uint32_t
#define TIFF_UINT64_T uint64_t
#define TIFF_UINT8_T uint8_t
#if defined(_WIN64) || defined(__LP64__) || (defined(__SIZEOF_POINTER__) && __SIZEOF_POINTER__ == 8)
#  define TIFF_SSIZE_T int64_t
#else
#  define TIFF_SSIZE_T int32_t
#endif

#define HAVE_IEEEFP 1
#define HOST_FILLORDER FILLORDER_LSB2MSB
#if defined(__BYTE_ORDER__) && __BYTE_ORDER__ == __ORDER_BIG_ENDIAN__
#  define HOST_BIGENDIAN 1
#else
#  define HOST_BIGENDIAN 0
#endif

/* the codecs: built-in ones, Deflate (zlib), and JPEG with the kit's libjpeg-turbo */
#define CCITT_SUPPORT 1
#define LOGLUV_SUPPORT 1
#define LZW_SUPPORT 1
#define NEXT_SUPPORT 1
#define PACKBITS_SUPPORT 1
#define THUNDER_SUPPORT 1
#define ZIP_SUPPORT 1
#ifdef BUI_IMAGE_JPEG
#  define JPEG_SUPPORT 1
#endif

#define STRIPCHOP_DEFAULT TIFF_STRIPCHOP
#define SUBIFD_SUPPORT 1
#define DEFAULT_EXTRASAMPLE_AS_ALPHA 1
#define CHECK_JPEG_YCBCR_SUBSAMPLING 1
#define MDI_SUPPORT 1
#define COLORIMETRY_SUPPORT
#define YCBCR_SUPPORT
#define CMYK_SUPPORT
#define ICC_SUPPORT
#define PHOTOSHOP_SUPPORT
#define IPTC_SUPPORT

#endif /* _TIFFCONF_ */
