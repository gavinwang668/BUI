/* TIFF through libtiff. Blender's imbuf (format_tiff.cc) reads TIFF through OpenImageIO, which
 * reads it through libtiff; here libtiff's RGBA interface (TIFFRGBAImage, as Blender's own
 * pre-OIIO tiff.cc used) converts the first page of any photometric it knows (RGB, palette,
 * grayscale, CMYK, YCbCr, CIE L*a*b*, LogLuv; 1 to 16 bits, strips or tiles, associated or
 * unassociated alpha) to premultiplied pixels. Codecs: none, LZW, PackBits, Deflate (zlib),
 * CCITT fax, NeXT, ThunderScan, LogLuv, and JPEG with the kit's libjpeg-turbo.
 * The Orientation tag is applied by imb_load, like JPEG's EXIF.
 *
 * The file is read from memory (TIFFClientOpenExt on the encoded bytes, mapped: strips are
 * read in place). Hostile input is bounded by libtiff's allocation limits, set per file, and
 * by the size checks of imb_alloc before the pixels are allocated.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */

#include <stdio.h> /* SEEK_* */
#include <string.h>

#include "MEM_guardedalloc.h"

#include "image.h"

#include "tiffio.h"

bool imb_is_a_tiff(const uint8_t *mem, size_t size)
{
  /* Blender's imb_is_a_tiff: classic ("II*\0", "MM\0*") and BigTIFF ("II+\0", "MM\0+") */
  return size >= 8 && ((mem[0] == 'I' && mem[1] == 'I' && (mem[2] == 42 || mem[2] == 43) &&
                        mem[3] == 0) ||
                       (mem[0] == 'M' && mem[1] == 'M' && mem[2] == 0 &&
                        (mem[3] == 42 || mem[3] == 43)));
}

/* -------------------------------------------------------------------- */
/** \name The file in memory (libtiff's client procs)
 * \{ */

typedef struct TiffMem {
  const uint8_t *mem;
  toff_t size, pos;
} TiffMem;

static tmsize_t tiff_read(thandle_t h, void *buf, tmsize_t n)
{
  TiffMem *m = (TiffMem *)h;
  const toff_t left = m->pos < m->size ? m->size - m->pos : 0;
  const toff_t k = n < 0 ? 0 : BK_MIN((toff_t)n, left);
  memcpy(buf, m->mem + m->pos, (size_t)k);
  m->pos += k;
  return (tmsize_t)k;
}

static tmsize_t tiff_write(thandle_t h, void *buf, tmsize_t n)
{
  (void)h;
  (void)buf;
  (void)n;
  return 0; /* read-only */
}

static toff_t tiff_seek(thandle_t h, toff_t off, int whence)
{
  TiffMem *m = (TiffMem *)h;
  const toff_t base = whence == SEEK_CUR ? m->pos : whence == SEEK_END ? m->size : 0;
  if (off > UINT64_MAX - base) {
    return (toff_t)-1;
  }
  m->pos = base + off; /* past the end reads nothing */
  return m->pos;
}

static int tiff_close(thandle_t h)
{
  (void)h;
  return 0;
}

static toff_t tiff_size(thandle_t h)
{
  return ((TiffMem *)h)->size;
}

static int tiff_map(thandle_t h, void **r_base, toff_t *r_size)
{
  TiffMem *m = (TiffMem *)h;
  *r_base = (void *)m->mem; /* libtiff reads mapped files, never writes them */
  *r_size = m->size;
  return 1;
}

static void tiff_unmap(thandle_t h, void *base, toff_t size)
{
  (void)h;
  (void)base;
  (void)size;
}

/* The first page of `m` (read-only), or NULL. */
static TIFF *tiff_open(TiffMem *m)
{
  TIFFOpenOptions *opts = TIFFOpenOptionsAlloc();
  if (!opts) {
    return NULL;
  }
  /* what libtiff allocates for one file (strips, tiles, tables): the pixels are imb_alloc's */
  TIFFOpenOptionsSetMaxSingleMemAlloc(opts, (tmsize_t)256 << 20);
  TIFFOpenOptionsSetMaxCumulatedMemAlloc(opts, (tmsize_t)512 << 20);
  TIFF *tif = TIFFClientOpenExt("bui", "r", (thandle_t)m, tiff_read, tiff_write, tiff_seek,
                                tiff_close, tiff_size, tiff_map, tiff_unmap, opts);
  TIFFOpenOptionsFree(opts);
  return tif;
}

/** \} */

bool imb_header_tiff(const uint8_t *mem, size_t size, BKImageInfo *r_info)
{
  TiffMem m = {mem, size, 0};
  TIFF *tif = tiff_open(&m);
  uint32_t w = 0, h = 0;
  const bool ok = tif && TIFFGetField(tif, TIFFTAG_IMAGEWIDTH, &w) &&
                  TIFFGetField(tif, TIFFTAG_IMAGELENGTH, &h) && w > 0 && h > 0 &&
                  w <= BK_IMB_MAX_SIDE && h <= BK_IMB_MAX_SIDE;
  if (tif) {
    TIFFClose(tif);
  }
  if (!ok) {
    return false;
  }
  const bool swap = imb_tiff_orientation(mem, size) >= 5;
  r_info->width = (int)(swap ? h : w);
  r_info->height = (int)(swap ? w : h);
  r_info->frames = 1; /* the first page */
  return true;
}

BKImBuf *imb_load_tiff(const uint8_t *mem, size_t size, int want_w, int want_h)
{
  (void)want_w; /* no cheaper smaller decode: imb_finish scales */
  (void)want_h;
  TiffMem m = {mem, size, 0};
  TIFF *tif = tiff_open(&m);
  if (!tif) {
    return NULL;
  }
  BKImBuf *ib = NULL;
  TIFFRGBAImage img;
  char emsg[1024];
  if (TIFFRGBAImageOK(tif, emsg) && TIFFRGBAImageBegin(&img, tif, 0, emsg)) {
    /* the rows as stored: imb_load applies the Orientation tag (all eight; libtiff's own
     * orienting flips only) */
    img.req_orientation = img.orientation;
    ib = imb_alloc((int)img.width, (int)img.height, 1);
    if (ib && !TIFFRGBAImageGet(&img, ib->rect, img.width, img.height)) {
      BK_imbuf_free(ib);
      ib = NULL;
    }
    TIFFRGBAImageEnd(&img);
  }
  TIFFClose(tif);
  if (ib) {
    /* libtiff's ABGR words (TIFFGetR: the low byte), premultiplied -> 0xAARRGGBB */
    const size_t n = (size_t)ib->x * ib->y;
    for (size_t i = 0; i < n; i++) {
      const uint32_t c = ib->rect[i];
      ib->rect[i] = (c & 0xFF00FF00u) | (c & 0xFFu) << 16 | (c >> 16 & 0xFFu);
    }
  }
  return ib;
}
