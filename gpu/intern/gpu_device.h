/* BUI GPU renderer core: the half of a GPU renderer that does not depend on the graphics API.
 *
 * gpu_device.c implements the whole private interface gpu_raster.c draws through
 * (gpu_d3d11.h): recording instances, triangles, meshes and effect draws into batches, the
 * shelf-packed glyph and image atlases and their caches, Slug glyph bands, big images and video
 * streams, the texture and effect registries' device copies, the persistent frame target's size
 * and the damage of each present. What only a graphics API can do (making textures, compiling
 * effects, drawing the batches, presenting) is a backend's BuiGpuOps table:
 *  - gpu_d3d11.c: Direct3D 11 (the default on Windows);
 *  - gpu_d3d12.c: Direct3D 12 (opt-in: BUI_gpu_set_api, BUI_GPU_API).
 * One device is one backend for its whole life: the table is picked when the device is created
 * (BUI_gpu_api, GPU_raster.h), and Direct3D 12 falls back to Direct3D 11 when it cannot start.
 * Blender splits its GPU module the same way: gpu/intern holds what every backend shares and
 * gpu/opengl, gpu/vulkan, gpu/metal fill a GPUBackend (gpu_backend.hh) of device objects.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */
#pragma once

#ifndef WIN32_LEAN_AND_MEAN
#  define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>

#include <stddef.h>
#include <stdint.h>

#include "gpu_d3d11.h"
#include "gpu_effect.h"

/* A backend's texture (with the views it is drawn through) and compiled effect. */
typedef struct BuiGpuTex BuiGpuTex;
typedef struct BuiGpuProg BuiGpuProg;

/* Texture formats the core asks for. */
typedef enum BuiGpuFormat {
  BUI_FMT_R8 = 0,  /* text masks, NV12 luma */
  BUI_FMT_RG8,     /* NV12 chroma */
  BUI_FMT_BGRA8,   /* 0xAARRGGBB pixels (images) */
  BUI_FMT_RGBA8,   /* registry textures, BUI_TARGET_RGBA8 */
  BUI_FMT_RGBA16F, /* BUI_TARGET_RGBA16F */
  BUI_FMT_RGBA32F, /* BUI_TARGET_RGBA32F, Slug curves */
  BUI_FMT_RG32UI,  /* Slug bands */
} BuiGpuFormat;

enum {
  BUI_TEX_IMMUTABLE = 1, /* never updated after creation */
  BUI_TEX_TARGET = 2,    /* effects draw into it; it starts transparent black */
};

/* What a backend's present reports. */
enum { BUI_PRESENT_OK = 0, BUI_PRESENT_OCCLUDED = 1, BUI_PRESENT_LOST = 2 };

typedef struct BuiGpuOps {
  BuiGpuApi api;
  /* Worker thread: the device and its pipeline (sets g->dev and g->max_tex). 0 when it cannot
   * start; `exit` then frees what was made. */
  int (*init)(BuiGpu *g);
  /* Frees g->dev and everything it holds. The core frees its textures and effects first. */
  void (*exit)(BuiGpu *g);
  /* A w x h texture, filled from `data` (rows `pitch` bytes apart) or, without, undefined (a
   * BUI_TEX_TARGET one: cleared). NULL when the device cannot make it. */
  BuiGpuTex *(*tex_create)(BuiGpu *g, BuiGpuFormat fmt, int w, int h, const void *data, int pitch,
                           int flags);
  /* Replaces a rect of `t` (drawn after what was flushed before, before what is recorded after). */
  void (*tex_update)(BuiGpu *g, BuiGpuTex *t, int x, int y, int w, int h, const void *data,
                     int pitch);
  void (*tex_free)(BuiGpu *g, BuiGpuTex *t);
  /* Compiles effect `v` (gpu_effect.h). NULL with the compiler's text in `log` when it fails. */
  BuiGpuProg *(*fx_build)(BuiGpu *g, const BuiEffectView *v, char *log, size_t log_n);
  void (*fx_free)(BuiGpu *g, BuiGpuProg *p);
  /* The persistent frame target at tw x th (remade only when its size changes), cleared to
   * transparent (alpha frames) or opaque black. 0 when it cannot be made. */
  int (*target)(BuiGpu *g, int tw, int th);
  /* Draws the recorded batches (BuiGpu.b ...) into the frame target, in order. */
  void (*draw)(BuiGpu *g);
  /* Sets a rect of the frame target to transparent. */
  void (*clear)(BuiGpu *g, const RECT *r);
  /* The window's swap chain at cw x ch, made or resized. 0 when there is none. */
  int (*swap)(BuiGpu *g, int cw, int ch);
  /* Copies the frame (whole: its w x h; else the `copy` rects) into the swap chain's buffer and
   * presents it, with `dirty` rects (n_dirty 0: everything changed). BUI_PRESENT_*. */
  int (*present)(BuiGpu *g, int whole, const RECT *copy, int n_copy, const RECT *dirty,
                 int n_dirty);
  /* Ends a frame that is not presented (no window): its work goes to the GPU. NULL: nothing to do. */
  void (*submit)(BuiGpu *g);
  /* The frame target's w x h top-left as 0xAARRGGBB rows into `dst`. 0 when it fails. */
  int (*read)(BuiGpu *g, uint32_t *dst, int w, int h);
} BuiGpuOps;

extern const BuiGpuOps bui_gpu_d3d11_ops;
extern const BuiGpuOps bui_gpu_d3d12_ops;

/* -------------------------------------------------------------------- */
/** \name Recorded draws
 * \{ */

/* Atlases: shelf packing in one texture. */
#define BUI_GPU_SHELVES 128
typedef struct BuiGpuAtlas {
  int size;
  int bpp; /* 1 (R8) or 4 (BGRA8) */
  BuiGpuTex *tex;
  int shelf_y[BUI_GPU_SHELVES], shelf_h[BUI_GPU_SHELVES], shelf_x[BUI_GPU_SHELVES];
  int nshelves, next_y;
} BuiGpuAtlas;

/* cache entry: key -> placement */
typedef struct BuiGpuSlot {
  uint64_t k1, k2; /* 0 = empty */
  int x, y;
} BuiGpuSlot;

typedef struct BuiGpuSlugGlyph {
  uint32_t key; /* (face + 1) << 16 | glyph; 0 = empty */
  int ok;       /* 0: empty glyph */
  float bbox[4], band[4];
  int loc[4];
} BuiGpuSlugGlyph;

typedef enum BuiGpuBatchKind {
  BUI_BATCH_QUADS = 0, /* instances (BuiGpuInst) */
  BUI_BATCH_TRIS = 1,  /* colored triangles (BuiGpuTriV) */
  BUI_BATCH_MESH = 2,  /* a textured mesh (BuiMeshVertex, indexed) */
  BUI_BATCH_FX = 3,    /* one effect draw or target clear (first: its BuiGpuFxCall) */
} BuiGpuBatchKind;

typedef struct BuiGpuBatch {
  BuiGpuBatchKind kind;
  int first, count;
  int img; /* quads: the RGBA texture their images sample (-1 none yet, 0 atlas, n: big[n - 1]) */
  /* meshes: texture (0: white), sampler, scissor x0 y0 x1 y1 */
  int tex, nearest;
  int clip[4];
} BuiGpuBatch;

/* An image too big for the atlas, or a video frame, in a texture of its own. */
#define BUI_GPU_BIG_MAX 32
typedef struct BuiGpuBig {
  uint64_t key, base; /* 0 = empty */
  int w, h;
  int format;    /* BUI_PIXMAP_*: NV12 is luma in tex, chroma in uv */
  int stream;    /* `base` is a stream: its frames update the textures in place */
  unsigned used; /* the frame it was last drawn in */
  BuiGpuTex *tex, *uv;
} BuiGpuBig;

/* A registry texture (BUI_texture_*) on this device, by registry slot. */
typedef struct BuiGpuDevTex {
  int id; /* 0 = empty */
  unsigned gen;
  int w, h;
  int target; /* a render target (BUI_target_create): 1 + BUI_TARGET_*, effects draw into it */
  BuiGpuTex *tex;
} BuiGpuDevTex;

typedef struct BuiGpuTriV {
  float x, y;
  float c[4];
  float clip[4];
} BuiGpuTriV;

/* An effect (gpu_effect.h) on this device, by registry slot: compiled on first use. */
typedef struct BuiGpuDevFx {
  int id;     /* 0 = empty */
  int failed; /* compiling it failed: the log is the effect's error */
  int n_inputs, n_floats;
  BuiGpuProg *prog;
} BuiGpuDevFx;

/* One recorded effect draw. */
typedef struct BuiGpuFxCall {
  int effect; /* 0: not a draw but a clear of `dst` (its 4 floats at `floats`) */
  int dst;    /* 0: the frame; else the render target it draws into */
  float rect[4];
  int inputs[BUI_EFFECT_MAX_INPUTS];
  int clip[4];
  int floats; /* where its n_floats of the effect are in BuiGpu.fxf */
} BuiGpuFxCall;

/* An effect's constant buffer (b1): box, 2 / target size, then one register per float (HLSL gives
 * each element of an array a register). Its size in bytes is bui_gpu_fx_cb_size. */
typedef struct BuiGpuFxConsts {
  float data[8 + 4 * BUI_EFFECT_MAX_FLOATS];
} BuiGpuFxConsts;

/** \} */

struct BuiGpu {
  const BuiGpuOps *ops;
  void *dev; /* the backend's device */
  HWND hwnd;
  /* alpha: frames keep premultiplied alpha (the target starts transparent) */
  int alpha;
  volatile LONG state; /* 0 pending, 1 ready, -1 failed */
  HANDLE thread;

  int has_target; /* ops->target made it */
  int tw, th;     /* target texture size (>= the frame) */
  int fw, fh;     /* the frame size the target holds */
  int w, h;       /* frame size */
  int has_swap, sw, sh; /* swap chain size */
  int swap_fresh; /* the swap chain was (re)made or its last frame dropped: present it whole */
  int max_tex;    /* the device's largest texture side */

  BuiGpuTex *white; /* 1x1 white: meshes without a texture, unbound inputs */
  BuiGpuDevTex *dtex;
  int n_dtex;
  unsigned dtex_free_gen;
  BuiGpuDevFx *dfx;
  int n_dfx;
  unsigned dfx_free_gen;

  BuiGpuAtlas mask, rgba;
  BuiGpuBig big[BUI_GPU_BIG_MAX];
  size_t big_bytes;
  unsigned frame, forget_seen;
  BuiGpuSlot *mask_map, *img_map;
  int mask_n, img_n;
  unsigned mask_gen;

  int slug; /* -1: off (unavailable), 0: not created yet, 1: ready */
  BuiGpuTex *curve_tex, *band_tex;
  BuiGpuSlugGlyph *slug_map;
  int slug_n, curve_next, band_next;
  unsigned slug_gen;

  BuiGpuInst *inst;
  int n_inst, cap_inst;
  BuiGpuTriV *tv;
  int n_tv, cap_tv;
  BuiMeshVertex *mv;
  int n_mv, cap_mv;
  uint32_t *mi;
  int n_mi, cap_mi;
  BuiGpuFxCall *fx;
  int n_fx, cap_fx;
  float *fxf;
  int n_fxf, cap_fxf;
  BuiGpuBatch *b;
  int n_b, cap_b;

  RECT dmg[16];
  int n_dmg, dmg_full;
  /* the last present's damage (whole: the whole frame): with two flip buffers, the buffer being
   * drawn is two frames old, so it needs this frame's damage and that one's, not everything */
  RECT prev[16];
  int n_prev, prev_whole;
};

/* -------------------------------------------------------------------- */
/** \name What the backends share
 * \{ */

/* BUI_GPU_DEBUG: 0 off, 1 report why the GPU path wasn't taken (and which device it took), 2 also
 * the API's debug layer (Direct3D 12) with its messages. */
int bui_gpu_debug(void);
/* Reports (BUI_GPU_DEBUG) that `what` failed with `hr`; returns 0. */
int bui_gpu_fail(const char *what, long hr);

/* The device texture of registry texture `id` once recorded (NULL: not live here). */
const BuiGpuDevTex *bui_gpu_dtex(const BuiGpu *g, int id);
/* The texture a recorded draw samples for registry texture `id` (g->white when not live). */
BuiGpuTex *bui_gpu_dtex_tex(const BuiGpu *g, int id);
/* The compiled effect a recorded draw runs (NULL: none). */
const BuiGpuDevFx *bui_gpu_dfx(const BuiGpu *g, int effect);
/* The vertex data of batch kind `k` (instances, triangle vertices, mesh vertices) and its count. */
const void *bui_gpu_batch_data(const BuiGpu *g, BuiGpuBatchKind k, int *r_count);
/* The atlas or big image texture (and NV12 chroma, else NULL) quad batch `b` samples. */
BuiGpuTex *bui_gpu_batch_image(const BuiGpu *g, const BuiGpuBatch *b, BuiGpuTex **r_uv);
/* An effect call's constant buffer for a target of tw x th; returns its size in bytes. */
size_t bui_gpu_fx_consts(const BuiGpu *g, const BuiGpuFxCall *fc, int n_floats, int tw, int th,
                         BuiGpuFxConsts *r_consts);

/** \} */
