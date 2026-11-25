/* SVG through the nanosvg the kit already carries for Blender's icons
 * (kit/extern/nanosvg, Blender's copy): parsed, then rasterized at the size
 * it is drawn (vector: no downscale needed). BUI_IMAGE_SVG (CMake option,
 * ON) puts it in the format table; the icons use imb_svg_rasterize always.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "MEM_guardedalloc.h"

#include "image.h"

#include "nanosvg.h"
#include "nanosvgrast.h"

bool imb_is_a_svg(const uint8_t *mem, size_t size)
{
  /* XML (optionally with a BOM) whose first kilobyte opens an <svg> element */
  size_t p = size >= 3 && mem[0] == 0xEF && mem[1] == 0xBB && mem[2] == 0xBF ? 3 : 0;
  while (p < size && (mem[p] == ' ' || mem[p] == '\t' || mem[p] == '\r' || mem[p] == '\n')) {
    p++;
  }
  if (p >= size || mem[p] != '<') {
    return false;
  }
  const size_t n = BK_MIN(size, (size_t)1024);
  for (size_t i = p; i + 4 <= n; i++) {
    if (memcmp(mem + i, "<svg", 4) == 0) {
      return true;
    }
  }
  return false;
}

/* The root <svg> element's start tag: [*r_from, *r_to) of `mem`, after any prolog, comments and
 * doctype. False when there is none. */
static bool root_tag(const uint8_t *mem, size_t size, size_t *r_from, size_t *r_to)
{
  for (size_t i = 0; i + 4 <= size; i++) {
    if (mem[i] != '<') {
      continue;
    }
    if (i + 4 <= size && memcmp(mem + i, "<!--", 4) == 0) {
      const uint8_t *end = NULL;
      for (size_t k = i + 4; k + 3 <= size; k++) {
        if (memcmp(mem + k, "-->", 3) == 0) {
          end = mem + k;
          break;
        }
      }
      if (!end) {
        return false;
      }
      i = (size_t)(end - mem) + 2;
      continue;
    }
    if (memcmp(mem + i, "<svg", 4) == 0 && i + 4 < size &&
        (mem[i + 4] == ' ' || mem[i + 4] == '\t' || mem[i + 4] == '\r' || mem[i + 4] == '\n' ||
         mem[i + 4] == '>' || mem[i + 4] == '/'))
    {
      size_t k = i + 4;
      char quote = 0;
      for (; k < size; k++) {
        if (quote) {
          quote = mem[k] == quote ? 0 : quote;
        }
        else if (mem[k] == '"' || mem[k] == '\'') {
          quote = (char)mem[k];
        }
        else if (mem[k] == '>') {
          break;
        }
      }
      if (k >= size) {
        return false;
      }
      *r_from = i;
      *r_to = k;
      return true;
    }
  }
  return false;
}

/* Attribute `name` of the tag [from, to): its value's start and length (the bytes between the
 * quotes); false when it has none. */
static bool tag_attr(const uint8_t *mem, size_t from, size_t to, const char *name, size_t *r_at,
                     size_t *r_len, size_t *r_attr_from, size_t *r_attr_to)
{
  const size_t n = strlen(name);
  for (size_t k = from + 4; k + n + 1 < to; k++) {
    const bool start = mem[k - 1] == ' ' || mem[k - 1] == '\t' || mem[k - 1] == '\r' ||
                       mem[k - 1] == '\n';
    if (!start || memcmp(mem + k, name, n) != 0) {
      continue;
    }
    size_t v = k + n;
    while (v < to && (mem[v] == ' ' || mem[v] == '\t' || mem[v] == '\r' || mem[v] == '\n')) {
      v++;
    }
    if (v >= to || mem[v] != '=') {
      continue;
    }
    v++;
    while (v < to && (mem[v] == ' ' || mem[v] == '\t' || mem[v] == '\r' || mem[v] == '\n')) {
      v++;
    }
    if (v >= to || (mem[v] != '"' && mem[v] != '\'')) {
      continue;
    }
    const uint8_t q = mem[v];
    size_t e = v + 1;
    while (e < to && mem[e] != q) {
      e++;
    }
    *r_at = v + 1;
    *r_len = e - v - 1;
    *r_attr_from = k;
    *r_attr_to = BK_MIN(e + 1, to);
    return true;
  }
  return false;
}

/* A length attribute in CSS px; false when missing, a percentage, or not a positive length. */
static bool attr_length(const uint8_t *mem, size_t from, size_t to, const char *name, float *r_px)
{
  size_t at, len, af, at_end;
  if (!tag_attr(mem, from, to, name, &at, &len, &af, &at_end) || len == 0 || len >= 64) {
    return false;
  }
  char buf[64];
  memcpy(buf, mem + at, len);
  buf[len] = 0;
  char *unit;
  const float v = strtof(buf, &unit);
  while (*unit == ' ') {
    unit++;
  }
  static const struct {
    const char *unit;
    float px;
  } units[] = {{"", 1.0f}, {"px", 1.0f}, {"pt", 96.0f / 72.0f}, {"pc", 16.0f}, {"in", 96.0f},
               {"cm", 96.0f / 2.54f}, {"mm", 96.0f / 25.4f}, {"em", 16.0f}, {"ex", 8.0f}};
  for (size_t i = 0; i < sizeof(units) / sizeof(units[0]); i++) {
    if (strncmp(unit, units[i].unit, 2) == 0 && strlen(unit) == strlen(units[i].unit)) {
      *r_px = v * units[i].px;
      return v > 0.0f;
    }
  }
  return false; /* a percentage, or junk */
}

/* The root's viewBox size; false without a usable one. */
static bool view_box(const uint8_t *mem, size_t from, size_t to, float *r_w, float *r_h)
{
  size_t at, len, af, at_end;
  if (!tag_attr(mem, from, to, "viewBox", &at, &len, &af, &at_end) || len >= 128) {
    return false;
  }
  char buf[128];
  memcpy(buf, mem + at, len);
  buf[len] = 0;
  float v[4] = {0};
  char *s = buf;
  for (int i = 0; i < 4; i++) {
    while (*s == ' ' || *s == ',' || *s == '\t' || *s == '\n' || *s == '\r') {
      s++;
    }
    char *end;
    v[i] = strtof(s, &end);
    if (end == s) {
      return false;
    }
    s = end;
  }
  *r_w = v[2];
  *r_h = v[3];
  return v[2] > 0.0f && v[3] > 0.0f;
}

/* nanosvg parses in place: a NUL-terminated copy. With `w` x `h` > 0, the root's size is set to
 * that (its viewBox, or one of its natural size when it has none, then fills it as its
 * preserveAspectRatio says), as an SVG image is drawn into the box it is given. */
static NSVGimage *parse(const uint8_t *mem, size_t size, int w, int h)
{
  size_t from = 0, to = 0;
  const bool sized = w > 0 && h > 0 && root_tag(mem, size, &from, &to);
  char extra[160] = "";
  if (sized) {
    float nw, nh, vw, vh;
    const bool natural = attr_length(mem, from, to, "width", &nw) && attr_length(mem, from, to, "height", &nh);
    if (!view_box(mem, from, to, &vw, &vh)) {
      /* none: one of its own size (it scales into the box), else the box's (it doesn't; nanosvg
       * also resolves percentages against the viewBox, so there must be one) */
      snprintf(extra, sizeof(extra), " viewBox=\"0 0 %g %g\"", natural ? (double)nw : (double)w,
               natural ? (double)nh : (double)h);
    }
    snprintf(extra + strlen(extra), sizeof(extra) - strlen(extra), " width=\"%dpx\" height=\"%dpx\"", w, h);
  }
  char *text = (char *)MEM_mallocN(size + strlen(extra) + 1, "svg text");
  if (!text) {
    return NULL;
  }
  memcpy(text, mem, size);
  text[size] = 0;
  if (sized) {
    /* the root's own width and height go (blanked: same length), the box's are added after <svg */
    const char *names[] = {"width", "height"};
    for (int i = 0; i < 2; i++) {
      size_t at, len, af, at_end;
      if (tag_attr((const uint8_t *)text, from, to, names[i], &at, &len, &af, &at_end)) {
        memset(text + af, ' ', at_end - af);
      }
    }
    memmove(text + from + 4 + strlen(extra), text + from + 4, size - from - 4 + 1);
    memcpy(text + from + 4, extra, strlen(extra));
  }
  NSVGimage *image = nsvgParse(text, "px", 96.0f);
  MEM_freeN(text);
  if (image && (image->width <= 0.0f || image->height <= 0.0f)) {
    nsvgDelete(image);
    return NULL;
  }
  return image;
}

bool imb_header_svg(const uint8_t *mem, size_t size, BKImageInfo *r_info)
{
  size_t from, to;
  if (!root_tag(mem, size, &from, &to)) {
    return false;
  }
  /* CSS's natural size: the root's width and height unless percentages (or missing); else the
   * viewBox's ratio; else none (300 x 150 stands in) */
  float w = 0.0f, h = 0.0f, vw = 0.0f, vh = 0.0f;
  const bool has_w = attr_length(mem, from, to, "width", &w);
  const bool has_h = attr_length(mem, from, to, "height", &h);
  const bool ratio = view_box(mem, from, to, &vw, &vh);
  int flags = BK_IMAGE_INFO_VECTOR;
  if (has_w && !has_h) {
    h = ratio ? w * vh / vw : 150.0f;
    flags |= ratio ? 0 : BK_IMAGE_INFO_NO_HEIGHT | BK_IMAGE_INFO_NO_RATIO;
  }
  else if (has_h && !has_w) {
    w = ratio ? h * vw / vh : 300.0f;
    flags |= ratio ? 0 : BK_IMAGE_INFO_NO_WIDTH | BK_IMAGE_INFO_NO_RATIO;
  }
  else if (!has_w && !has_h) {
    flags |= BK_IMAGE_INFO_NO_WIDTH | BK_IMAGE_INFO_NO_HEIGHT | (ratio ? 0 : BK_IMAGE_INFO_NO_RATIO);
    w = ratio ? vw : 300.0f;
    h = ratio ? vh : 150.0f;
  }
  r_info->width = BK_MAX(1, (int)ceilf(w));
  r_info->height = BK_MAX(1, (int)ceilf(h));
  r_info->frames = 1;
  r_info->flags = flags;
  return true;
}

static BKImBuf *rasterize(NSVGimage *image, int want_w, int want_h)
{
  float scale = 1.0f;
  if (want_w > 0 && want_h > 0) {
    scale = fminf((float)want_w / image->width, (float)want_h / image->height);
  }
  const int w = BK_MAX(1, (int)ceilf(image->width * scale));
  const int h = BK_MAX(1, (int)ceilf(image->height * scale));
  scale = (float)w / image->width;
  BKImBuf *ib = imb_alloc(w, h, 1);
  NSVGrasterizer *rast = ib ? nsvgCreateRasterizer() : NULL;
  if (!rast) {
    BK_imbuf_free(ib);
    return NULL;
  }
  /* straight RGBA into the buffer, then premultiplied BGRA in place */
  nsvgRasterize(rast, image, 0.0f, 0.0f, scale, (unsigned char *)ib->rect, w, h, w * 4);
  nsvgDeleteRasterizer(rast);
  imb_premultiply_rgba(ib->rect, (const uint8_t *)ib->rect, w * h);
  return ib;
}

BKImBuf *imb_load_svg(const uint8_t *mem, size_t size, int want_w, int want_h)
{
  /* drawn into a want_w x want_h box: parsed at that size, rasterized 1:1 */
  NSVGimage *image = parse(mem, size, want_w, want_h);
  BKImBuf *ib = image ? rasterize(image, want_w, want_h) : NULL;
  if (image) {
    nsvgDelete(image);
  }
  return ib;
}

BKImBuf *imb_svg_rasterize(const char *svg, int want_w, int want_h)
{
  NSVGimage *image = parse((const uint8_t *)svg, strlen(svg), 0, 0);
  BKImBuf *ib = image ? rasterize(image, want_w, want_h) : NULL;
  if (image) {
    nsvgDelete(image);
  }
  return ib;
}
