/* AVIF through libavif with dav1d (C, no assembly), as Blender's imbuf (format_avif.cc, which
 * reads it through OpenImageIO's libheif/libavif): stills, grids and alpha decode into the
 * BKImBuf as premultiplied BGRA (libavif's YUV -> RGB with alphaPremultiplied), any bit depth
 * and chroma subsampling; the irot / imir properties are applied by imb_load like JPEG's EXIF
 * orientation. Image sequences ('avis') play through imb_avif_codec, one frame at a time, as
 * animated WebP.
 *
 * Decodes run on the kit's workers, several at once: each has its own decoder, single-threaded
 * (maxThreads 1: dav1d starts no threads of its own). Hostile input: libavif's size limits are
 * the kit's (BK_IMB_MAX_SIDE), checked when the file is parsed, before anything is decoded.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */

#include <string.h>

#include "MEM_guardedalloc.h"

#include "image.h"

#include "avif/avif.h"

static const char *const avif_brands[] = {"avif", "avis", NULL};

bool imb_is_a_avif(const uint8_t *mem, size_t size)
{
  return imb_isobmff_brand(mem, size, avif_brands);
}

bool imb_avif_animated(const uint8_t *mem, size_t size)
{
  static const char *const seq[] = {"avis", NULL};
  return imb_isobmff_brand(mem, size, seq);
}

/* A decoder on `mem` (kept by the caller while it is open), parsed: NULL when it is no AVIF
 * this build reads, or larger than the kit's limits. */
static avifDecoder *avif_open(const uint8_t *mem, size_t size)
{
  avifDecoder *dec = avifDecoderCreate();
  if (!dec) {
    return NULL;
  }
  dec->maxThreads = 1;
  dec->imageDimensionLimit = BK_IMB_MAX_SIDE;
  dec->imageSizeLimit = (uint32_t)BK_MIN(BK_IMB_MAX_PIXELS, (size_t)AVIF_DEFAULT_IMAGE_SIZE_LIMIT);
  dec->strictFlags = AVIF_STRICT_DISABLED; /* what browsers show, they show */
  dec->ignoreExif = AVIF_TRUE;
  dec->ignoreXMP = AVIF_TRUE;
  if (avifDecoderSetIOMemory(dec, mem, size) != AVIF_RESULT_OK ||
      avifDecoderParse(dec) != AVIF_RESULT_OK || dec->image->width == 0 ||
      dec->image->height == 0 || dec->image->width > BK_IMB_MAX_SIDE ||
      dec->image->height > BK_IMB_MAX_SIDE)
  {
    avifDecoderDestroy(dec);
    return NULL;
  }
  return dec;
}

/* irot (anticlockwise quarter turns) then imir, as the EXIF orientation imb_orient applies. */
static int avif_orientation(const avifImage *im)
{
  static const int table[4][3] = {{1, 4, 2}, {8, 5, 7}, {3, 2, 4}, {6, 7, 5}};
  const int rot = (im->transformFlags & AVIF_TRANSFORM_IROT) ? im->irot.angle & 3 : 0;
  const int mir = (im->transformFlags & AVIF_TRANSFORM_IMIR) ? 1 + (im->imir.axis & 1) : 0;
  return table[rot][mir];
}

int imb_avif_orientation(const uint8_t *mem, size_t size)
{
  avifDecoder *dec = avif_open(mem, size);
  const int o = dec ? avif_orientation(dec->image) : 1;
  avifDecoderDestroy(dec);
  return o;
}

bool imb_header_avif(const uint8_t *mem, size_t size, BKImageInfo *r_info)
{
  avifDecoder *dec = avif_open(mem, size);
  if (!dec) {
    return false;
  }
  const bool swap = avif_orientation(dec->image) >= 5;
  r_info->width = (int)(swap ? dec->image->height : dec->image->width);
  r_info->height = (int)(swap ? dec->image->width : dec->image->height);
  r_info->frames = BK_MAX(1, dec->imageCount);
  avifDecoderDestroy(dec);
  return true;
}

/* The decoded frame into `dst` (its size), premultiplied 0xAARRGGBB. */
static bool avif_to_argb(avifImage *im, uint32_t *dst)
{
  avifRGBImage rgb;
  avifRGBImageSetDefaults(&rgb, im);
  rgb.format = AVIF_RGB_FORMAT_BGRA; /* premultiplied 0xAARRGGBB on little-endian */
  rgb.depth = 8;
  rgb.alphaPremultiplied = AVIF_TRUE;
  rgb.maxThreads = 1;
  rgb.pixels = (uint8_t *)dst;
  rgb.rowBytes = im->width * 4;
  return avifImageYUVToRGB(im, &rgb) == AVIF_RESULT_OK;
}

BKImBuf *imb_load_avif(const uint8_t *mem, size_t size, int want_w, int want_h)
{
  (void)want_w; /* full size: imb_finish scales */
  (void)want_h;
  avifDecoder *dec = avif_open(mem, size);
  if (!dec) {
    return NULL;
  }
  BKImBuf *ib = NULL;
  if (avifDecoderNextImage(dec) == AVIF_RESULT_OK) {
    ib = imb_alloc((int)dec->image->width, (int)dec->image->height, 1);
    if (ib && !avif_to_argb(dec->image, ib->rect)) {
      BK_imbuf_free(ib);
      ib = NULL;
    }
  }
  avifDecoderDestroy(dec);
  return ib;
}

/* -------------------------------------------------------------------- */
/** \name Image sequences, one frame at a time (anim.c)
 * \{ */

typedef struct AvifAnim {
  avifDecoder *dec;
  int w, h;
} AvifAnim;

static void avif_close(void *state)
{
  AvifAnim *a = (AvifAnim *)state;
  avifDecoderDestroy(a->dec);
  MEM_freeN(a);
}

static void *avif_anim_open(const char *path, const uint8_t *mem, size_t size,
                            BKImAnimInfo *r_info)
{
  (void)path;
  avifDecoder *dec = avif_open(mem, size);
  if (!dec) {
    return NULL;
  }
  AvifAnim *a = (AvifAnim *)MEM_callocN(sizeof(AvifAnim), "AvifAnim");
  a->dec = dec;
  r_info->w = r_info->src_w = a->w = (int)dec->image->width;
  r_info->h = r_info->src_h = a->h = (int)dec->image->height;
  r_info->format = BUI_PIXMAP_ARGB;
  /* repetitions after the first play; infinite or unknown: forever */
  r_info->loops = dec->repetitionCount >= 0 ? dec->repetitionCount + 1 : 0;
  r_info->duration = dec->duration;
  return a;
}

static int avif_next(void *state, BKImBuf *dst, double *r_pts, double *r_dur)
{
  AvifAnim *a = (AvifAnim *)state;
  const avifResult r = avifDecoderNextImage(a->dec);
  if (r == AVIF_RESULT_NO_IMAGES_REMAINING) {
    return 0;
  }
  /* every frame of a sequence has the track's size: one that hasn't is an error */
  if (r != AVIF_RESULT_OK || (int)a->dec->image->width != a->w ||
      (int)a->dec->image->height != a->h || !avif_to_argb(a->dec->image, dst->rect))
  {
    return -1;
  }
  *r_pts = a->dec->imageTiming.pts;
  *r_dur = a->dec->imageTiming.duration;
  return 1;
}

static bool avif_seek(void *state, double t)
{
  (void)t; /* from the top: anim.c skips to t */
  return avifDecoderReset(((AvifAnim *)state)->dec) == AVIF_RESULT_OK;
}

static size_t avif_bytes(const void *state)
{
  const AvifAnim *a = (const AvifAnim *)state;
  /* the decoder's frame (YUV, up to 16 bits a sample) and dav1d's references, estimated */
  return sizeof(*a) + 8 * sizeof(uint32_t) * (size_t)a->w * a->h;
}

const BKImAnimCodec imb_avif_codec = {"avif", false, imb_avif_animated, avif_anim_open, NULL,
                                      avif_next, avif_seek, avif_bytes, avif_close};

/** \} */
