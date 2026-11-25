/* HEIC on Linux and the BSDs (format_heic.c): libheif, loaded at run time (dlopen; no headers
 * or link), with whatever HEVC decoder plugin the system gave it (libde265, FFmpeg). libheif
 * applies the file's rotation and mirroring itself, so the primary image's size is the shown
 * one; pixels come as straight RGBA, premultiplied here. Its size limit is the kit's, and it
 * decodes on the calling thread (no decoding threads of its own: the kit's workers run several
 * decodes at once).
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */

#ifdef BUI_HEIC_LIBHEIF

#  include <dlfcn.h>
#  include <pthread.h>
#  include <string.h>

#  include "image.h"

/* libheif's C API (heif.h), the part used here: stable since libheif 1.0 */
struct heif_context;
struct heif_image_handle;
struct heif_image;
struct heif_error {
  int code; /* heif_error_Ok = 0 */
  int subcode;
  const char *message;
};
enum { HEIF_COLORSPACE_RGB = 1, HEIF_CHROMA_INTERLEAVED_RGBA = 11, HEIF_CHANNEL_INTERLEAVED = 10 };
enum { HEIF_COMPRESSION_HEVC = 1 };

static struct {
  pthread_once_t once;
  bool ok;
  struct heif_context *(*context_alloc)(void);
  void (*context_free)(struct heif_context *);
  struct heif_error (*read_from_memory_without_copy)(struct heif_context *, const void *, size_t,
                                                     const void *);
  struct heif_error (*get_primary_image_handle)(struct heif_context *,
                                                struct heif_image_handle **);
  int (*handle_get_width)(const struct heif_image_handle *);
  int (*handle_get_height)(const struct heif_image_handle *);
  void (*handle_release)(const struct heif_image_handle *);
  struct heif_error (*decode_image)(const struct heif_image_handle *, struct heif_image **, int,
                                    int, const void *);
  const uint8_t *(*get_plane_readonly)(const struct heif_image *, int, int *);
  int (*image_get_width)(const struct heif_image *, int);
  int (*image_get_height)(const struct heif_image *, int);
  void (*image_release)(const struct heif_image *);
  /* optional (newer libheif) */
  void (*set_max_image_size)(struct heif_context *, int);
  void (*set_max_threads)(struct heif_context *, int);
  int (*have_decoder)(int);
} H = {PTHREAD_ONCE_INIT};

static void heif_load(void)
{
  void *lib = dlopen("libheif.so.1", RTLD_NOW | RTLD_LOCAL);
  if (!lib) {
    lib = dlopen("libheif.so", RTLD_NOW | RTLD_LOCAL);
  }
  if (!lib) {
    return; /* no libheif: no HEIC, nothing else changes */
  }
#  define SYM(field, name) *(void **)&H.field = dlsym(lib, name)
  SYM(context_alloc, "heif_context_alloc");
  SYM(context_free, "heif_context_free");
  SYM(read_from_memory_without_copy, "heif_context_read_from_memory_without_copy");
  SYM(get_primary_image_handle, "heif_context_get_primary_image_handle");
  SYM(handle_get_width, "heif_image_handle_get_width");
  SYM(handle_get_height, "heif_image_handle_get_height");
  SYM(handle_release, "heif_image_handle_release");
  SYM(decode_image, "heif_decode_image");
  SYM(get_plane_readonly, "heif_image_get_plane_readonly");
  SYM(image_get_width, "heif_image_get_width");
  SYM(image_get_height, "heif_image_get_height");
  SYM(image_release, "heif_image_release");
  SYM(set_max_image_size, "heif_context_set_maximum_image_size_limit");
  SYM(set_max_threads, "heif_context_set_max_decoding_threads");
  SYM(have_decoder, "heif_have_decoder_for_format");
#  undef SYM
  H.ok = H.context_alloc && H.context_free && H.read_from_memory_without_copy &&
         H.get_primary_image_handle && H.handle_get_width && H.handle_get_height &&
         H.handle_release && H.decode_image && H.get_plane_readonly && H.image_get_width &&
         H.image_get_height && H.image_release &&
         (!H.have_decoder || H.have_decoder(HEIF_COMPRESSION_HEVC));
}

bool imb_heic_supported(void)
{
  pthread_once(&H.once, heif_load);
  return H.ok;
}

/* The primary image of `mem` (kept by the caller), within the kit's limits; NULL on failure.
 * `*r_ctx`: free it after the handle. */
static struct heif_image_handle *heif_open(const uint8_t *mem, size_t size,
                                           struct heif_context **r_ctx, int *r_w, int *r_h)
{
  *r_ctx = imb_heic_supported() ? H.context_alloc() : NULL;
  if (!*r_ctx) {
    return NULL;
  }
  if (H.set_max_image_size) {
    H.set_max_image_size(*r_ctx, BK_IMB_MAX_SIDE);
  }
  if (H.set_max_threads) {
    H.set_max_threads(*r_ctx, 0); /* decode on this thread */
  }
  struct heif_image_handle *h = NULL;
  if (H.read_from_memory_without_copy(*r_ctx, mem, size, NULL).code != 0 ||
      H.get_primary_image_handle(*r_ctx, &h).code != 0)
  {
    return NULL;
  }
  *r_w = H.handle_get_width(h);
  *r_h = H.handle_get_height(h);
  if (*r_w <= 0 || *r_h <= 0 || *r_w > BK_IMB_MAX_SIDE || *r_h > BK_IMB_MAX_SIDE) {
    H.handle_release(h);
    return NULL;
  }
  return h;
}

static void heif_close(struct heif_context *ctx, struct heif_image_handle *h)
{
  if (h) {
    H.handle_release(h);
  }
  if (ctx) {
    H.context_free(ctx);
  }
}

bool imb_header_heic(const uint8_t *mem, size_t size, BKImageInfo *r_info)
{
  struct heif_context *ctx;
  struct heif_image_handle *h = heif_open(mem, size, &ctx, &r_info->width, &r_info->height);
  heif_close(ctx, h);
  r_info->frames = 1;
  return h != NULL;
}

BKImBuf *imb_load_heic(const uint8_t *mem, size_t size, int want_w, int want_h)
{
  (void)want_w; /* full size: imb_finish scales */
  (void)want_h;
  struct heif_context *ctx;
  int w, h;
  struct heif_image_handle *handle = heif_open(mem, size, &ctx, &w, &h);
  struct heif_image *img = NULL;
  BKImBuf *ib = NULL;
  if (handle && H.decode_image(handle, &img, HEIF_COLORSPACE_RGB, HEIF_CHROMA_INTERLEAVED_RGBA,
                               NULL)
                        .code == 0)
  {
    int stride = 0;
    const uint8_t *px = H.get_plane_readonly(img, HEIF_CHANNEL_INTERLEAVED, &stride);
    const int iw = H.image_get_width(img, HEIF_CHANNEL_INTERLEAVED);
    const int ih = H.image_get_height(img, HEIF_CHANNEL_INTERLEAVED);
    /* the decoded image is the handle's size; anything else is not trusted */
    if (px && iw == w && ih == h && stride >= w * 4 && (ib = imb_alloc(w, h, 1))) {
      for (int y = 0; y < h; y++) {
        imb_premultiply_rgba(ib->rect + (size_t)y * w, px + (size_t)y * stride, w);
      }
    }
  }
  if (img) {
    H.image_release(img);
  }
  heif_close(ctx, handle);
  return ib;
}

#endif /* BUI_HEIC_LIBHEIF */
