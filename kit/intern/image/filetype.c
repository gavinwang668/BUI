/* The format table and the load path: Blender's IMB_FILE_TYPES
 * (imbuf/intern/filetype.cc) and IMB_load_image_from_memory
 * (imbuf/intern/readimage.cc). Every decode, synchronous or on the
 * workers, goes through imb_load().
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */

#include <limits.h>
#include <math.h>
#include <stdio.h>
#include <string.h>

#ifdef _WIN32
#  define WIN32_LEAN_AND_MEAN
#  include <windows.h>
#endif

#include "MEM_guardedalloc.h"

#include "image.h"

/* The formats this build has (features.json: BUI_IMAGE_*). Order matters
 * only for TGA, which has no signature: it goes last. */
const BKImFileType BK_IMB_FILE_TYPES[] = {
#ifdef BUI_IMAGE_PNG
    {BK_IMAGE_PNG, "png", imb_is_a_png, imb_header_wuffs, imb_load_wuffs, NULL},
#endif
#ifdef BUI_IMAGE_JPEG
    {BK_IMAGE_JPEG, "jpeg", imb_is_a_jpeg, imb_header_jpeg, imb_load_jpeg, imb_jpeg_orientation},
#endif
#ifdef BUI_IMAGE_WEBP
    {BK_IMAGE_WEBP, "webp", imb_is_a_webp, imb_header_webp, imb_load_webp, NULL},
#endif
#ifdef BUI_IMAGE_GIF
    {BK_IMAGE_GIF, "gif", imb_is_a_gif, imb_header_wuffs, imb_load_wuffs, NULL},
#endif
#ifdef BUI_IMAGE_BMP
    {BK_IMAGE_BMP, "bmp", imb_is_a_bmp, imb_header_wuffs, imb_load_wuffs, NULL},
#endif
#ifdef BUI_IMAGE_QOI
    {BK_IMAGE_QOI, "qoi", imb_is_a_qoi, imb_header_wuffs, imb_load_wuffs, NULL},
#endif
#ifdef BUI_IMAGE_HDR
    {BK_IMAGE_HDR, "hdr", imb_is_a_hdr, imb_header_hdr, imb_load_hdr, NULL},
#endif
#ifdef BUI_IMAGE_PNM
    {BK_IMAGE_PNM, "pnm", imb_is_a_pnm, imb_header_pnm, imb_load_pnm, NULL},
#endif
#ifdef BUI_IMAGE_TIFF
    {BK_IMAGE_TIFF, "tiff", imb_is_a_tiff, imb_header_tiff, imb_load_tiff, imb_tiff_orientation},
#endif
#ifdef BUI_IMAGE_EXR
    {BK_IMAGE_EXR, "exr", imb_is_a_exr, imb_header_exr, imb_load_exr, NULL},
#endif
#ifdef BUI_IMAGE_AVIF
    {BK_IMAGE_AVIF, "avif", imb_is_a_avif, imb_header_avif, imb_load_avif, imb_avif_orientation},
#endif
#ifdef BUI_IMAGE_JXL
    {BK_IMAGE_JXL, "jxl", imb_is_a_jxl, imb_header_jxl, imb_load_jxl, NULL},
#endif
#ifdef BUI_IMAGE_HEIC
    {BK_IMAGE_HEIC, "heic", imb_is_a_heic, imb_header_heic, imb_load_heic, NULL},
#endif
#ifdef BUI_IMAGE_SVG
    {BK_IMAGE_SVG, "svg", imb_is_a_svg, imb_header_svg, imb_load_svg, NULL},
#endif
#ifdef BUI_IMAGE_TGA
    {BK_IMAGE_TGA, "tga", imb_is_a_tga, imb_header_wuffs, imb_load_wuffs, NULL},
#endif
    {BK_IMAGE_NONE, NULL, NULL, NULL, NULL, NULL}, /* the end (a build may have no formats) */
};
const int BK_IMB_FILE_TYPES_NUM = (int)(sizeof(BK_IMB_FILE_TYPES) / sizeof(BK_IMB_FILE_TYPES[0])) - 1;

bool imb_isobmff_brand(const uint8_t *mem, size_t size, const char *const *brands)
{
  /* the first box is 'ftyp': size, 'ftyp', major brand, minor version, compatible brands */
  if (size < 16 || memcmp(mem + 4, "ftyp", 4) != 0) {
    return false;
  }
  const size_t box = (size_t)mem[0] << 24 | (size_t)mem[1] << 16 | (size_t)mem[2] << 8 | mem[3];
  const size_t end = BK_MIN(box, size);
  for (size_t p = 8; p + 4 <= end; p += p == 8 ? 8 : 4) { /* major, then compatible brands */
    for (int i = 0; brands[i]; i++) {
      if (memcmp(mem + p, brands[i], 4) == 0) {
        return true;
      }
    }
  }
  return false;
}

const BKImFileType *imb_file_type(const uint8_t *mem, size_t size)
{
  if (!mem || size < 4) {
    return NULL;
  }
  for (int i = 0; i < BK_IMB_FILE_TYPES_NUM; i++) {
    if (BK_IMB_FILE_TYPES[i].is_a(mem, size)) {
      return &BK_IMB_FILE_TYPES[i];
    }
  }
  return NULL;
}

const char *BK_image_format_name(int format)
{
  for (int i = 0; i < BK_IMB_FILE_TYPES_NUM; i++) {
    if (BK_IMB_FILE_TYPES[i].ftype == format) {
      return BK_IMB_FILE_TYPES[i].name;
    }
  }
  return NULL;
}

/* The header (its size the shown one: after the file's orientation). */
static const BKImFileType *probe(const uint8_t *mem, size_t size, BKImageInfo *r_info)
{
  const BKImFileType *t = imb_file_type(mem, size);
  memset(r_info, 0, sizeof(*r_info));
  if (!t || !t->load_header(mem, size, r_info) || r_info->width <= 0 || r_info->height <= 0 ||
      r_info->width > BK_IMB_MAX_SIDE || r_info->height > BK_IMB_MAX_SIDE)
  {
    return NULL;
  }
  r_info->format = t->ftype;
  r_info->frames = BK_MAX(1, r_info->frames);
  return t;
}

int BK_image_probe(const void *data, size_t len, BKImageInfo *r_info)
{
  BKImageInfo info;
  const bool ok = probe((const uint8_t *)data, len, &info) != NULL;
  if (r_info) {
    *r_info = info;
  }
  return ok;
}

bool imb_plan(const BKImageFit *t, int iw, int ih, int orient, bool vector, BKImPlan *r)
{
  float dst[4];
  bk_image_fit(t, iw, ih, dst, r->src);
  if (dst[2] < 0.5f || dst[3] < 0.5f) {
    return false; /* nothing of it shows */
  }
  /* The visible part at the size it is drawn, never upscaled (drawing
   * upscales) unless a vector draws it, and the whole image at that scale
   * (what the decoders see). */
  const float *src = r->src;
  const int cap_w = vector ? BK_IMB_MAX_SIDE : BK_MAX(1, (int)ceilf(src[2]));
  const int cap_h = vector ? BK_IMB_MAX_SIDE : BK_MAX(1, (int)ceilf(src[3]));
  const int ow = BK_CLAMP((int)lroundf(dst[2]), 1, cap_w);
  const int oh = BK_CLAMP((int)lroundf(dst[3]), 1, cap_h);
  const double fx = ow / (double)src[2], fy = oh / (double)src[3];
  const double kx = vector ? fx : BK_MIN(1.0, fx), ky = vector ? fy : BK_MIN(1.0, fy);
  r->iw = iw;
  r->ih = ih;
  r->orient = orient;
  r->fw = BK_CLAMP((int)lround(iw * kx), 1, BK_IMB_MAX_SIDE);
  r->fh = BK_CLAMP((int)lround(ih * ky), 1, BK_IMB_MAX_SIDE);
  return true;
}

/* Scale steps of the whole decodes, per doubling: a decode is at most 2^(1/4) (1.19) times the size
 * drawn, so a box resized within a step draws the same decode (no decoder runs) at a scale the
 * bilinear draw keeps sharp. */
#define SCALE_STEPS 4.0

bool imb_whole_fit(const BKImageInfo *info, const BKImageFit *want, BKImageFit *r_whole)
{
  float dst[4], src[4];
  bk_image_fit(want, info->width, info->height, dst, src);
  if (dst[2] < 0.5f || dst[3] < 0.5f) {
    return false; /* nothing of it shows */
  }
#define STEP(s) exp2(ceil(log2(s) * SCALE_STEPS - 1e-6) / SCALE_STEPS)
  if (info->flags & BK_IMAGE_INFO_VECTOR) {
    /* a vector at the scale of each axis (an SVG stretched or letterboxed into a box of another
     * ratio is drawn into that box, not scaled from its own), above its own size too: exactly
     * (rasterizing costs little; scaled from another size its edges blur) */
    const double sx = dst[2] / src[2], sy = dst[3] / src[3];
    *r_whole = (BKImageFit){BK_FIT_FILL, BK_CLAMP((int)lround(info->width * sx), 1, BK_IMB_MAX_SIDE),
                            BK_CLAMP((int)lround(info->height * sy), 1, BK_IMB_MAX_SIDE), 0.5f, 0.5f,
                            want->frame};
    return true;
  }
  /* the largest scale it is drawn at (fill stretches), as a step; never above intrinsic: the
   * decoders never upscale (imb_plan), so a larger step would only be another entry of the same
   * pixels. Drawing upscales. */
  const double scale = fmax(dst[2] / src[2], dst[3] / src[3]);
  const double cap = fmin(1.0, STEP(scale));
#undef STEP
  *r_whole = (BKImageFit){BK_FIT_FILL, BK_MAX(1, (int)lround(info->width * cap)),
                          BK_MAX(1, (int)lround(info->height * cap)), 0.5f, 0.5f, want->frame};
  return true;
}

BKImBuf *imb_finish(BKImBuf *ib, const BKImPlan *p)
{
  const bool swap = p->orient >= 5;
  ib = imb_scale(ib, swap ? p->fh : p->fw, swap ? p->fw : p->fh);
  ib = imb_orient(ib, p->orient);
  /* the visible part, in whole pixels covering it, and its exact position
   * inside them (upscaled draws then sample the right sub-pixel region) */
  const double kx = ib->x / (double)p->iw, ky = ib->y / (double)p->ih;
  const double cx = p->src[0] * kx, cy = p->src[1] * ky, cw = p->src[2] * kx, ch = p->src[3] * ky;
  const int px = (int)floor(cx + 1e-6), py = (int)floor(cy + 1e-6);
  const int pw = BK_MAX(1, BK_MIN(ib->x, (int)ceil(cx + cw - 1e-6)) - px);
  const int ph = BK_MAX(1, BK_MIN(ib->y, (int)ceil(cy + ch - 1e-6)) - py);
  ib = imb_crop(ib, px, py, pw, ph);
  ib->src_x = p->iw;
  ib->src_y = p->ih;
  ib->crop[0] = (float)(cx - px);
  ib->crop[1] = (float)(cy - py);
  ib->crop[2] = (float)BK_MIN(cw, (double)ib->x - ib->crop[0]);
  ib->crop[3] = (float)BK_MIN(ch, (double)ib->y - ib->crop[1]);
  return ib;
}

BKImBuf *imb_load(const uint8_t *mem, size_t size, const BKImageFit *target)
{
  BKImageInfo info;
  const BKImFileType *t = probe(mem, size, &info);
  if (!t) {
    return NULL;
  }
  if (imb_anim_codec(mem, size)) { /* GIF, animated WebP, AVIF sequences, animated JPEG XL */
    return target->frame > 0 ? imb_anim_frame(mem, size, target, target->frame - 1) :
                               imb_anim_all(mem, size, target);
  }
  const int orient = t->orientation ? t->orientation(mem, size) : 1;
  BKImPlan plan;
  /* a vector decodes above its own size only for a box it is drawn into (the cache's whole decodes
   * fill theirs); BK_imbuf_decode fits within and never upscales */
  const bool vector = (info.flags & BK_IMAGE_INFO_VECTOR) && target->fit == BK_FIT_FILL;
  if (!imb_plan(target, info.width, info.height, orient, vector, &plan)) {
    return NULL;
  }
  const bool swap = orient >= 5;
  BKImBuf *ib = t->load(mem, size, swap ? plan.fh : plan.fw, swap ? plan.fw : plan.fh);
  if (!ib) {
    return NULL;
  }
  ib->ftype = t->ftype;
  ib = imb_finish(ib, &plan);
  if (ib) {
    ib->src_flags = info.flags;
  }
  return ib;
}

/* -------------------------------------------------------------------- */
/** \name Files
 * \{ */

uint8_t *imb_read_file(const char *path, size_t max, size_t *r_size)
{
  *r_size = 0;
  if (!path || !path[0]) {
    return NULL;
  }
#ifdef _WIN32
  wchar_t wpath[MAX_PATH * 4];
  if (!MultiByteToWideChar(CP_UTF8, 0, path, -1, wpath, (int)(sizeof(wpath) / sizeof(wpath[0])))) {
    return NULL;
  }
  HANDLE h = CreateFileW(wpath, GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE, NULL,
                         OPEN_EXISTING, FILE_FLAG_SEQUENTIAL_SCAN, NULL);
  if (h == INVALID_HANDLE_VALUE) {
    return NULL;
  }
  LARGE_INTEGER li;
  uint8_t *buf = NULL;
  if (GetFileSizeEx(h, &li) && li.QuadPart > 0 && (uint64_t)li.QuadPart < ((uint64_t)1 << 31)) {
    size_t n = (size_t)li.QuadPart;
    n = max && max < n ? max : n;
    DWORD got = 0;
    buf = (uint8_t *)MEM_mallocN(n, "image file");
    if (buf && ReadFile(h, buf, (DWORD)n, &got, NULL) && got == n) {
      *r_size = n;
    }
    else {
      MEM_freeN(buf);
      buf = NULL;
    }
  }
  CloseHandle(h);
  return buf;
#else
  FILE *f = fopen(path, "rb");
  if (!f) {
    return NULL;
  }
  uint8_t *buf = NULL;
  long len = -1;
  if (fseek(f, 0, SEEK_END) == 0 && (len = ftell(f)) > 0 && fseek(f, 0, SEEK_SET) == 0 &&
      len < INT_MAX)
  {
    size_t n = (size_t)len;
    n = max && max < n ? max : n;
    buf = (uint8_t *)MEM_mallocN(n, "image file");
    if (buf && fread(buf, 1, n, f) == n) {
      *r_size = n;
    }
    else {
      MEM_freeN(buf);
      buf = NULL;
    }
  }
  fclose(f);
  return buf;
#endif
}

int BK_image_probe_file(const char *path, BKImageInfo *r_info)
{
  /* the header is near the start; JPEGs with big EXIF/ICC blocks need more */
  size_t n;
  uint8_t *mem = imb_read_file(path, 64 * 1024, &n);
  int ok = BK_image_probe(mem, n, r_info);
  if (!ok && mem && n == 64 * 1024) {
    MEM_freeN(mem);
    mem = imb_read_file(path, 0, &n);
    ok = BK_image_probe(mem, n, r_info);
  }
  MEM_freeN(mem);
  return ok;
}

/** \} */

/* -------------------------------------------------------------------- */
/** \name Synchronous decode (BUI_kit.h)
 * \{ */

BKImageFit imb_fit_within(int max_w, int max_h)
{
  /* contain within max_w x max_h; 0 leaves that side free */
  BKImageFit f = {BK_FIT_CONTAIN, max_w, max_h, 0.5f, 0.5f, 0};
  if (max_w > 0 || max_h > 0) {
    f.box_w = max_w > 0 ? max_w : BK_IMB_MAX_SIDE;
    f.box_h = max_h > 0 ? max_h : BK_IMB_MAX_SIDE;
  }
  return f;
}

BKImBuf *BK_imbuf_decode(const void *data, size_t len, int max_w, int max_h)
{
  const BKImageFit f = imb_fit_within(max_w, max_h);
  return imb_load((const uint8_t *)data, len, &f);
}

BKImBuf *BK_imbuf_decode_frame(const void *data, size_t len, int frame, int max_w, int max_h)
{
  BKImageFit f = imb_fit_within(max_w, max_h);
  f.frame = frame + 1;
  return frame >= 0 ? imb_load((const uint8_t *)data, len, &f) : NULL;
}

BKImBuf *BK_imbuf_load(const char *path, int max_w, int max_h)
{
  size_t n;
  uint8_t *mem = imb_read_file(path, 0, &n);
  BKImBuf *ib = mem ? BK_imbuf_decode(mem, n, max_w, max_h) : NULL;
  MEM_freeN(mem);
  return ib;
}

/** \} */
