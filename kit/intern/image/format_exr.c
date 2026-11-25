/* OpenEXR through OpenEXRCore, the C library of OpenEXR 3 (Blender's imbuf reads EXR through
 * the same project's C++ API: openexr/openexr_api.cpp). The first part's R, G, B and A (or Y,
 * grayscale), of the plain layer or else the first named one ("diffuse.R"), scanlines or tiles
 * (the full-resolution level), half, float or uint, in every compression but zstd and HTJ2K
 * (extern/openexr/README.bui). Its pixels are scene-linear and premultiplied: they are shown the
 * way Blender's "Standard" view transform shows them (imb_srgb, as Radiance HDR), one chunk at
 * a time (no float copy of the whole image).
 *
 * Hostile input: OpenEXRCore validates the header and every chunk against the file; the
 * context's size limits are the kit's (BK_IMB_MAX_SIDE), and its memory comes from MEM_*.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */

#include <string.h>

#include "MEM_guardedalloc.h"

#include "image.h"

#include "openexr.h"

bool imb_is_a_exr(const uint8_t *mem, size_t size)
{
  /* the magic number 20000630, little-endian (Blender's imb_is_a_openexr) */
  return size >= 4 && mem[0] == 0x76 && mem[1] == 0x2f && mem[2] == 0x31 && mem[3] == 0x01;
}

/* -------------------------------------------------------------------- */
/** \name The file in memory
 * \{ */

typedef struct ExrMem {
  const uint8_t *mem;
  size_t size;
} ExrMem;

static int64_t exr_mem_read(exr_const_context_t ctxt, void *userdata, void *buffer, uint64_t sz,
                            uint64_t offset, exr_stream_error_func_ptr_t error_cb)
{
  (void)ctxt;
  (void)error_cb;
  const ExrMem *m = (const ExrMem *)userdata;
  if (offset >= m->size) {
    return 0;
  }
  const uint64_t n = BK_MIN(sz, (uint64_t)m->size - offset);
  memcpy(buffer, m->mem + offset, (size_t)n);
  return (int64_t)n;
}

static int64_t exr_mem_size(exr_const_context_t ctxt, void *userdata)
{
  (void)ctxt;
  return (int64_t)((const ExrMem *)userdata)->size;
}

static void exr_quiet(exr_const_context_t ctxt, exr_result_t code, const char *msg)
{
  (void)ctxt; /* a file that fails to decode fails quietly, like every format */
  (void)code;
  (void)msg;
}

static void *exr_alloc(size_t bytes)
{
  return MEM_mallocN(bytes, "OpenEXRCore");
}

/* A reading context on `m` with the kit's limits, or NULL. */
static exr_context_t exr_open(ExrMem *m)
{
  exr_context_initializer_t init = EXR_DEFAULT_CONTEXT_INITIALIZER;
  init.user_data = m;
  init.read_fn = exr_mem_read;
  init.size_fn = exr_mem_size;
  init.error_handler_fn = exr_quiet;
  init.alloc_fn = exr_alloc;
  init.free_fn = MEM_freeN;
  init.max_image_width = init.max_image_height = BK_IMB_MAX_SIDE;
  init.max_tile_width = init.max_tile_height = BK_IMB_MAX_SIDE;
  init.flags = EXR_CONTEXT_FLAG_SILENT_HEADER_PARSE;
  exr_context_t ctxt = NULL;
  if (exr_start_read(&ctxt, "bui", &init) != EXR_ERR_SUCCESS) {
    exr_finish(&ctxt);
    return NULL;
  }
  return ctxt;
}

/* Part 0's data window size: what is shown (Blender reads the data window too). */
static bool exr_size(exr_context_t ctxt, exr_attr_box2i_t *r_dw, int *r_w, int *r_h)
{
  if (exr_get_data_window(ctxt, 0, r_dw) != EXR_ERR_SUCCESS) {
    return false;
  }
  const int64_t w = (int64_t)r_dw->max.x - r_dw->min.x + 1;
  const int64_t h = (int64_t)r_dw->max.y - r_dw->min.y + 1;
  if (w <= 0 || h <= 0 || w > BK_IMB_MAX_SIDE || h > BK_IMB_MAX_SIDE) {
    return false;
  }
  *r_w = (int)w;
  *r_h = (int)h;
  return true;
}

/** \} */

bool imb_header_exr(const uint8_t *mem, size_t size, BKImageInfo *r_info)
{
  ExrMem m = {mem, size};
  exr_context_t ctxt = exr_open(&m);
  exr_attr_box2i_t dw;
  const bool ok = ctxt && exr_size(ctxt, &dw, &r_info->width, &r_info->height);
  exr_finish(&ctxt);
  r_info->frames = 1;
  return ok;
}

/* -------------------------------------------------------------------- */
/** \name Decoding
 * \{ */

enum { EXR_R, EXR_G, EXR_B, EXR_A, EXR_Y, EXR_NUM };

/* Which of R, G, B, A, Y a channel is: of the layer `prefix` (`plen` bytes; 0: no layer). */
static int exr_channel_slot(const char *name, const char *prefix, size_t plen)
{
  if (plen) {
    if (strncmp(name, prefix, plen) != 0 || name[plen] != '.') {
      return -1;
    }
    name += plen + 1;
  }
  static const char *const slots[EXR_NUM] = {"R", "G", "B", "A", "Y"};
  for (int s = 0; s < EXR_NUM; s++) {
    if (strcmp(name, slots[s]) == 0) {
      return s;
    }
  }
  return -1;
}

/* The layer to show: none when the file has plain R, G, B or Y; else the first one that has. */
static size_t exr_layer(const exr_attr_chlist_t *ch, const char **r_prefix)
{
  *r_prefix = NULL;
  for (int i = 0; i < ch->num_channels; i++) {
    const int s = exr_channel_slot(ch->entries[i].name.str, NULL, 0);
    if (s >= 0 && s != EXR_A) {
      return 0;
    }
  }
  for (int i = 0; i < ch->num_channels; i++) {
    const char *name = ch->entries[i].name.str, *dot = strrchr(name, '.');
    if (dot && exr_channel_slot(dot + 1, NULL, 0) >= 0 && dot[1] != 'A') {
      *r_prefix = name;
      return (size_t)(dot - name);
    }
  }
  return 0;
}

typedef struct ExrTarget {
  const char *prefix;
  size_t plen;
  float *buf; /* one chunk, RGBA float */
  size_t buf_floats;
  uint8_t lut[IMB_SRGB_STEPS + 1];
} ExrTarget;

/* Decode one chunk and write it to `ib` at (x0, y0) (data window relative). */
static bool exr_chunk(exr_context_t ctxt, const exr_chunk_info_t *cinfo, exr_decode_pipeline_t *dec,
                      bool *inited, ExrTarget *t, BKImBuf *ib, int x0, int y0)
{
  const int cw = cinfo->width, chh = cinfo->height;
  if (cw <= 0 || chh <= 0 || x0 < 0 || y0 < 0 || x0 >= ib->x || y0 >= ib->y) {
    return false;
  }
  const size_t floats = (size_t)cw * chh * 4;
  if (floats > t->buf_floats) {
    MEM_freeN(t->buf);
    t->buf = (float *)MEM_mallocN(floats * sizeof(float), "exr chunk");
    t->buf_floats = t->buf ? floats : 0;
    if (!t->buf) {
      return false;
    }
  }
  exr_result_t r = *inited ? exr_decoding_update(ctxt, 0, cinfo, dec) :
                             exr_decoding_initialize(ctxt, 0, cinfo, dec);
  if (r != EXR_ERR_SUCCESS) {
    return false;
  }
  *inited = true;
  bool slot_set[EXR_NUM] = {false};
  for (int c = 0; c < dec->channel_count; c++) {
    exr_coding_channel_info_t *ch = &dec->channels[c];
    int s = exr_channel_slot(ch->channel_name, t->prefix, t->plen);
    if (s == EXR_Y) {
      s = EXR_R; /* gray: spread below */
    }
    if (s < 0 || slot_set[s] || ch->x_samples != 1 || ch->y_samples != 1) {
      ch->decode_to_ptr = NULL; /* not shown (subsampled chroma: RY / BY) */
      continue;
    }
    slot_set[s] = true;
    ch->decode_to_ptr = (uint8_t *)(t->buf + s);
    ch->user_pixel_stride = 4 * sizeof(float);
    ch->user_line_stride = (int32_t)(4 * sizeof(float) * (size_t)cw);
    ch->user_data_type = EXR_PIXEL_FLOAT;
    ch->user_bytes_per_element = sizeof(float);
  }
  if (!slot_set[EXR_R] && !slot_set[EXR_G] && !slot_set[EXR_B]) {
    return false; /* nothing to show */
  }
  if (exr_decoding_choose_default_routines(ctxt, 0, dec) != EXR_ERR_SUCCESS ||
      exr_decoding_run(ctxt, 0, dec) != EXR_ERR_SUCCESS)
  {
    return false;
  }
  const bool gray = slot_set[EXR_R] && !slot_set[EXR_G] && !slot_set[EXR_B];
  const int w = BK_MIN(cw, ib->x - x0), h = BK_MIN(chh, ib->y - y0);
  for (int y = 0; y < h; y++) {
    const float *s = t->buf + (size_t)y * cw * 4;
    uint32_t *d = ib->rect + (size_t)(y0 + y) * ib->x + x0;
    for (int x = 0; x < w; x++, s += 4) {
      const float r = s[0], g = gray ? s[0] : slot_set[EXR_G] ? s[1] : 0.0f;
      const float b = gray ? s[0] : slot_set[EXR_B] ? s[2] : 0.0f;
      const float a = slot_set[EXR_A] ? s[3] : 1.0f;
      if (a >= 1.0f) {
        d[x] = 0xFF000000u | imb_srgb(t->lut, r) << 16 | imb_srgb(t->lut, g) << 8 |
               imb_srgb(t->lut, b);
      }
      else if (a > 0.0f) {
        /* premultiplied linear: straight, encoded, premultiplied again (8 bits) */
        const uint32_t a8 = (uint32_t)(a * 255.0f + 0.5f);
        const float k = 1.0f / a;
        const uint32_t cr = (imb_srgb(t->lut, r * k) * a8 + 127) / 255;
        const uint32_t cg = (imb_srgb(t->lut, g * k) * a8 + 127) / 255;
        const uint32_t cb = (imb_srgb(t->lut, b * k) * a8 + 127) / 255;
        d[x] = a8 << 24 | cr << 16 | cg << 8 | cb;
      }
      else {
        d[x] = 0; /* transparent (alpha 0 or NaN) */
      }
    }
  }
  return true;
}

BKImBuf *imb_load_exr(const uint8_t *mem, size_t size, int want_w, int want_h)
{
  (void)want_w; /* full resolution: imb_finish scales */
  (void)want_h;
  ExrMem m = {mem, size};
  exr_context_t ctxt = exr_open(&m);
  exr_attr_box2i_t dw;
  int w, h;
  exr_storage_t storage;
  const exr_attr_chlist_t *chlist = NULL;
  if (!ctxt || !exr_size(ctxt, &dw, &w, &h) ||
      exr_get_storage(ctxt, 0, &storage) != EXR_ERR_SUCCESS ||
      (storage != EXR_STORAGE_SCANLINE && storage != EXR_STORAGE_TILED) ||
      exr_get_channels(ctxt, 0, &chlist) != EXR_ERR_SUCCESS)
  {
    exr_finish(&ctxt);
    return NULL;
  }
  BKImBuf *ib = imb_alloc(w, h, 1);
  if (!ib) {
    exr_finish(&ctxt);
    return NULL;
  }
  memset(ib->rect, 0, sizeof(uint32_t) * (size_t)w * h); /* a chunk that fails: transparent */
  ExrTarget t = {NULL};
  t.plen = exr_layer(chlist, &t.prefix);
  imb_srgb_table(t.lut);
  exr_decode_pipeline_t dec = EXR_DECODE_PIPELINE_INITIALIZER;
  bool inited = false, any = false;
  exr_chunk_info_t cinfo;
  if (storage == EXR_STORAGE_SCANLINE) {
    int32_t lines = 0;
    if (exr_get_scanlines_per_chunk(ctxt, 0, &lines) == EXR_ERR_SUCCESS && lines > 0) {
      for (int64_t y = dw.min.y; y <= dw.max.y; y += lines) {
        if (exr_read_scanline_chunk_info(ctxt, 0, (int)y, &cinfo) == EXR_ERR_SUCCESS) {
          any |= exr_chunk(ctxt, &cinfo, &dec, &inited, &t, ib, 0,
                           (int)((int64_t)cinfo.start_y - dw.min.y));
        }
      }
    }
  }
  else {
    uint32_t tw = 0, th = 0;
    int32_t nx = 0, ny = 0;
    if (exr_get_tile_descriptor(ctxt, 0, &tw, &th, NULL, NULL) == EXR_ERR_SUCCESS &&
        exr_get_tile_counts(ctxt, 0, 0, 0, &nx, &ny) == EXR_ERR_SUCCESS && tw > 0 && th > 0)
    {
      for (int32_t ty = 0; ty < ny; ty++) {
        for (int32_t tx = 0; tx < nx; tx++) {
          if (exr_read_tile_chunk_info(ctxt, 0, tx, ty, 0, 0, &cinfo) == EXR_ERR_SUCCESS) {
            any |= exr_chunk(ctxt, &cinfo, &dec, &inited, &t, ib, (int)((int64_t)tx * tw),
                             (int)((int64_t)ty * th));
          }
        }
      }
    }
  }
  if (inited) {
    exr_decoding_destroy(ctxt, &dec);
  }
  MEM_freeN(t.buf);
  exr_finish(&ctxt);
  if (!any) {
    BK_imbuf_free(ib);
    return NULL;
  }
  return ib;
}

/** \} */
