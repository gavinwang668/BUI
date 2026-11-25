/* JPEG XL through libjxl's decoder (C API; the library is C++ with Highway). Blender reads JPEG
 * XL through OpenImageIO, which uses this library too. Stills decode to 8-bit RGBA (alpha made
 * straight by libjxl, premultiplied here), with the file's orientation applied by libjxl (the
 * header reports the shown size); XYB (lossy) images are asked for in sRGB, the others come in
 * the color space they were stored in. Animations play through imb_jxl_codec, one composited
 * frame at a time, as animated WebP.
 *
 * Decodes run on the kit's workers, several at once: each has its own decoder and no parallel
 * runner (single-threaded). Hostile input: the size from the basic info is checked against the
 * kit's limits before any frame is decoded, and libjxl's memory comes from MEM_*.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */

#include <string.h>

#include "MEM_guardedalloc.h"

#include "image.h"

#include "jxl/decode.h"

bool imb_is_a_jxl(const uint8_t *mem, size_t size)
{
  const JxlSignature s = JxlSignatureCheck(mem, size);
  return s == JXL_SIG_CODESTREAM || s == JXL_SIG_CONTAINER;
}

/* -------------------------------------------------------------------- */
/** \name The decoder
 * \{ */

static void *jxl_alloc(void *opaque, size_t size)
{
  (void)opaque;
  return MEM_mallocN(size, "libjxl");
}

static void jxl_free(void *opaque, void *address)
{
  (void)opaque;
  MEM_freeN(address);
}

/* A decoder on `mem` (kept by the caller), past its basic info: NULL when it is no JPEG XL or
 * larger than the kit's limits. r_w x r_h is the shown size (after orientation). */
static JxlDecoder *jxl_open(const uint8_t *mem, size_t size, int events, JxlBasicInfo *r_info,
                            int *r_w, int *r_h)
{
  static const JxlMemoryManager mm = {NULL, jxl_alloc, jxl_free};
  JxlDecoder *dec = JxlDecoderCreate(&mm);
  if (!dec) {
    return NULL;
  }
  if (JxlDecoderSubscribeEvents(dec, JXL_DEC_BASIC_INFO | events) == JXL_DEC_SUCCESS &&
      JxlDecoderSetUnpremultiplyAlpha(dec, JXL_TRUE) == JXL_DEC_SUCCESS &&
      JxlDecoderSetInput(dec, mem, size) == JXL_DEC_SUCCESS)
  {
    JxlDecoderCloseInput(dec);
    if (JxlDecoderProcessInput(dec) == JXL_DEC_BASIC_INFO &&
        JxlDecoderGetBasicInfo(dec, r_info) == JXL_DEC_SUCCESS && r_info->xsize > 0 &&
        r_info->ysize > 0 && r_info->xsize <= BK_IMB_MAX_SIDE && r_info->ysize <= BK_IMB_MAX_SIDE)
    {
      const bool swap = r_info->orientation >= JXL_ORIENT_TRANSPOSE;
      *r_w = (int)(swap ? r_info->ysize : r_info->xsize);
      *r_h = (int)(swap ? r_info->xsize : r_info->ysize);
      return dec;
    }
  }
  JxlDecoderDestroy(dec);
  return NULL;
}

/* The next frame into `dst` (w * h pixels, premultiplied 0xAARRGGBB) and its duration in
 * ticks: 1, 0 past the last, -1 on errors. */
static int jxl_frame(JxlDecoder *dec, const JxlBasicInfo *info, uint32_t *dst, int w, int h,
                     uint32_t *r_ticks)
{
  static const JxlPixelFormat fmt = {4, JXL_TYPE_UINT8, JXL_NATIVE_ENDIAN, 0};
  const size_t n = (size_t)w * h;
  for (;;) {
    switch (JxlDecoderProcessInput(dec)) {
      case JXL_DEC_COLOR_ENCODING:
        if (!info->uses_original_profile) {
          /* XYB: in sRGB (no CMS: libjxl converts XYB itself) */
          JxlColorEncoding srgb;
          memset(&srgb, 0, sizeof(srgb));
          srgb.color_space = info->num_color_channels == 1 ? JXL_COLOR_SPACE_GRAY :
                                                             JXL_COLOR_SPACE_RGB;
          srgb.white_point = JXL_WHITE_POINT_D65;
          srgb.primaries = JXL_PRIMARIES_SRGB;
          srgb.transfer_function = JXL_TRANSFER_FUNCTION_SRGB;
          srgb.rendering_intent = JXL_RENDERING_INTENT_PERCEPTUAL;
          JxlDecoderSetOutputColorProfile(dec, &srgb, NULL, 0);
        }
        break;
      case JXL_DEC_FRAME: {
        JxlFrameHeader fh;
        *r_ticks = JxlDecoderGetFrameHeader(dec, &fh) == JXL_DEC_SUCCESS ? fh.duration : 0;
        break;
      }
      case JXL_DEC_NEED_IMAGE_OUT_BUFFER: {
        size_t need = 0;
        if (JxlDecoderImageOutBufferSize(dec, &fmt, &need) != JXL_DEC_SUCCESS ||
            need != n * 4 || JxlDecoderSetImageOutBuffer(dec, &fmt, dst, n * 4) != JXL_DEC_SUCCESS)
        {
          return -1;
        }
        break;
      }
      case JXL_DEC_FULL_IMAGE:
        imb_premultiply_rgba(dst, (const uint8_t *)dst, (int)n); /* in place */
        return 1;
      case JXL_DEC_SUCCESS:
        return 0;
      default: /* JXL_DEC_ERROR, or a truncated file (JXL_DEC_NEED_MORE_INPUT) */
        return -1;
    }
  }
}

#define JXL_PIXEL_EVENTS (JXL_DEC_COLOR_ENCODING | JXL_DEC_FRAME | JXL_DEC_FULL_IMAGE)

/** \} */

bool imb_jxl_animated(const uint8_t *mem, size_t size)
{
  if (!imb_is_a_jxl(mem, size)) {
    return false;
  }
  JxlBasicInfo info;
  int w, h;
  JxlDecoder *dec = jxl_open(mem, size, 0, &info, &w, &h);
  const bool anim = dec && info.have_animation;
  JxlDecoderDestroy(dec);
  return anim;
}

bool imb_header_jxl(const uint8_t *mem, size_t size, BKImageInfo *r_info)
{
  JxlBasicInfo info;
  JxlDecoder *dec = jxl_open(mem, size, JXL_DEC_FRAME, &info, &r_info->width, &r_info->height);
  if (!dec) {
    return false;
  }
  r_info->frames = 1;
  if (info.have_animation) {
    /* the frames' headers (their pixels are skipped: no JXL_DEC_FULL_IMAGE) */
    int frames = 0;
    while (JxlDecoderProcessInput(dec) == JXL_DEC_FRAME) {
      frames++;
    }
    r_info->frames = BK_MAX(1, frames);
  }
  JxlDecoderDestroy(dec);
  return true;
}

BKImBuf *imb_load_jxl(const uint8_t *mem, size_t size, int want_w, int want_h)
{
  (void)want_w; /* full size: imb_finish scales */
  (void)want_h;
  JxlBasicInfo info;
  int w, h;
  JxlDecoder *dec = jxl_open(mem, size, JXL_PIXEL_EVENTS, &info, &w, &h);
  BKImBuf *ib = dec ? imb_alloc(w, h, 1) : NULL;
  uint32_t ticks;
  if (ib && jxl_frame(dec, &info, ib->rect, w, h, &ticks) != 1) {
    BK_imbuf_free(ib);
    ib = NULL;
  }
  JxlDecoderDestroy(dec);
  return ib;
}

/* -------------------------------------------------------------------- */
/** \name Animations, one frame at a time (anim.c)
 * \{ */

typedef struct JxlAnim {
  JxlDecoder *dec;
  const uint8_t *mem;
  size_t size;
  JxlBasicInfo info;
  int w, h;
  double tick; /* seconds */
} JxlAnim;

static void jxl_close(void *state)
{
  JxlAnim *a = (JxlAnim *)state;
  JxlDecoderDestroy(a->dec);
  MEM_freeN(a);
}

static void *jxl_anim_open(const char *path, const uint8_t *mem, size_t size,
                           BKImAnimInfo *r_info)
{
  (void)path;
  JxlAnim *a = (JxlAnim *)MEM_callocN(sizeof(JxlAnim), "JxlAnim");
  a->mem = mem;
  a->size = size;
  a->dec = jxl_open(mem, size, JXL_PIXEL_EVENTS, &a->info, &a->w, &a->h);
  if (!a->dec || !a->info.have_animation || a->info.animation.tps_numerator == 0) {
    jxl_close(a);
    return NULL;
  }
  a->tick = (double)a->info.animation.tps_denominator / a->info.animation.tps_numerator;
  r_info->w = r_info->src_w = a->w;
  r_info->h = r_info->src_h = a->h;
  r_info->format = BUI_PIXMAP_ARGB;
  r_info->loops = (int)BK_MIN(a->info.animation.num_loops, (uint32_t)INT32_MAX); /* 0: forever */
  return a;
}

static int jxl_next(void *state, BKImBuf *dst, double *r_pts, double *r_dur)
{
  (void)r_pts;
  JxlAnim *a = (JxlAnim *)state;
  uint32_t ticks = 0;
  const int r = jxl_frame(a->dec, &a->info, dst->rect, a->w, a->h, &ticks);
  *r_dur = ticks * a->tick;
  return r;
}

static bool jxl_seek(void *state, double t)
{
  (void)t; /* from the top: anim.c skips to t */
  JxlAnim *a = (JxlAnim *)state;
  JxlDecoderRewind(a->dec);
  if (JxlDecoderSetInput(a->dec, a->mem, a->size) != JXL_DEC_SUCCESS) {
    return false;
  }
  JxlDecoderCloseInput(a->dec);
  return JxlDecoderProcessInput(a->dec) == JXL_DEC_BASIC_INFO;
}

static size_t jxl_bytes(const void *state)
{
  const JxlAnim *a = (const JxlAnim *)state;
  /* the decoder's frame buffers (float planes) and the last composited frame, estimated */
  return sizeof(*a) + 6 * sizeof(float) * (size_t)a->w * a->h;
}

const BKImAnimCodec imb_jxl_codec = {"jxl", false, imb_jxl_animated, jxl_anim_open, NULL,
                                     jxl_next, jxl_seek, jxl_bytes, jxl_close};

/** \} */
