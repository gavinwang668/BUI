/* HEIC / HEIF: decoded by the OS, loaded at run time like <video> (movie_*.c), never vendored:
 * HEVC is patent-encumbered, and browsers leave it to the OS too. One backend per platform
 * (kit/CMakeLists.txt picks it): Windows Imaging Component with the HEIF and HEVC Video
 * Extensions (heic_wic.c), ImageIO on macOS (heic_imageio.c), libheif on Linux (heic_libheif.c,
 * dlopen'd). On a system without a decoder, HEIC files read as no image: the format is missing
 * there, as in a build without it.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */

#include "image.h"

bool imb_is_a_heic(const uint8_t *mem, size_t size)
{
  /* HEVC brands, and the generic HEIF ones unless the file is an AVIF */
  static const char *const heif[] = {"heic", "heix", "heim", "heis", "hevc", "hevx",
                                     "hevm", "hevs", "mif1", "msf1", NULL};
  static const char *const avif[] = {"avif", "avis", NULL};
  return imb_isobmff_brand(mem, size, heif) && !imb_isobmff_brand(mem, size, avif) &&
         imb_heic_supported();
}

#ifdef BUI_HEIC_NO_BACKEND /* a platform with none of the three: no HEIC */

bool imb_heic_supported(void)
{
  return false;
}

bool imb_header_heic(const uint8_t *mem, size_t size, BKImageInfo *r_info)
{
  (void)mem;
  (void)size;
  (void)r_info;
  return false;
}

BKImBuf *imb_load_heic(const uint8_t *mem, size_t size, int want_w, int want_h)
{
  (void)mem;
  (void)size;
  (void)want_w;
  (void)want_h;
  return NULL;
}

#endif
