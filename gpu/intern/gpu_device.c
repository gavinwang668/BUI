/* BUI GPU renderer core — see gpu_device.h.
 *
 * Like Blender's widget drawing (interface_widgets.cc batches widget bases
 * into GPU_SHADER_2D_WIDGET_BASE_INST draws), every primitive becomes one
 * instance of a single uber-shader, so a frame of thousands of widgets and
 * text runs is a handful of instanced draws. Differences from Blender,
 * for a retained UI:
 *  - frames render into a persistent target: only damaged regions are
 *    redrawn (the kit's damage tracking) and the swap chain gets dirty rects,
 *    so the compositor only recomposes what changed;
 *  - text is pre-shaped glyph runs (the same masks the software raster draws)
 *    packed into one R8 atlas, images into one BGRA atlas;
 *  - the device is created on a worker thread so the first frame never waits
 *    for the driver (the software raster draws until it is ready).
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */

#if defined(_WIN32) && defined(BUI_GPU) /* BUI_GPU: features.json */

#  include "gpu_device.h"

#  include <stdio.h>
#  include <stdlib.h>
#  include <string.h>

#  define MASK_MAP 16384 /* power of two */
#  define MASK_MAP_FILL 12000
#  define IMG_MAP 1024
#  define IMG_MAP_FILL 700

/* Slug glyph data: curves (2 RGBA32F texels each) and bands (RG32_UINT
 * texels), both addressed linearly over 4096-wide textures. */
#  define SLUG_W 4096
#  define SLUG_H 64
#  define SLUG_MAP 8192 /* power of two */
#  define SLUG_MAP_FILL 6000
#  define SLUG_MAX_BANDS 16

#  define BIG_BUDGET ((size_t)256 << 20) /* bytes of big-image textures kept */
#  define BIG_IDLE 120                   /* frames unused before one is freed */

int bui_gpu_debug(void)
{
  const char *dbg = getenv("BUI_GPU_DEBUG");
  return dbg && dbg[0] >= '1' && dbg[0] <= '9' ? dbg[0] - '0' : 0;
}

int bui_gpu_fail(const char *what, long hr)
{
  if (bui_gpu_debug()) {
    fprintf(stderr, "bui gpu: %s failed (0x%08lx)\n", what, (unsigned long)hr);
  }
  return 0;
}

static void tex_drop(BuiGpu *g, BuiGpuTex **t)
{
  if (*t) {
    g->ops->tex_free(g, *t);
    *t = NULL;
  }
}

/* -------------------------------------------------------------------- */
/** \name Atlases (shelf packing)
 * \{ */

static int atlas_create(BuiGpu *g, BuiGpuAtlas *a, int size, int bpp)
{
  memset(a, 0, sizeof(*a));
  a->size = size;
  a->bpp = bpp;
  a->tex = g->ops->tex_create(g, bpp == 1 ? BUI_FMT_R8 : BUI_FMT_BGRA8, size, size, NULL, 0, 0);
  return a->tex != NULL;
}

static void atlas_clear(BuiGpuAtlas *a)
{
  a->nshelves = 0;
  a->next_y = 0;
}

/* Finds room for w x h (+1 px gutter). 0 when full. */
static int atlas_place(BuiGpuAtlas *a, int w, int h, int *r_x, int *r_y)
{
  const int pw = w + 1, ph = h + 1;
  if (pw > a->size || ph > a->size) {
    return 0;
  }
  int best = -1;
  for (int i = 0; i < a->nshelves; i++) {
    if (a->shelf_h[i] >= ph && a->shelf_h[i] <= ph + ph / 4 + 2 && a->shelf_x[i] + pw <= a->size &&
        (best < 0 || a->shelf_h[i] < a->shelf_h[best]))
    {
      best = i;
    }
  }
  if (best < 0) {
    const int sh = (ph + 3) & ~3; /* round up: more strings share a shelf */
    if (a->nshelves == BUI_GPU_SHELVES || a->next_y + sh > a->size) {
      return 0;
    }
    best = a->nshelves++;
    a->shelf_y[best] = a->next_y;
    a->shelf_h[best] = sh;
    a->shelf_x[best] = 0;
    a->next_y += sh;
  }
  *r_x = a->shelf_x[best];
  *r_y = a->shelf_y[best];
  a->shelf_x[best] += pw;
  return 1;
}

/** \} */

/* -------------------------------------------------------------------- */
/** \name Device (worker thread)
 * \{ */

/* The device and what the core keeps on it from the start. */
static int device_init(BuiGpu *g)
{
  if (!g->ops->init(g)) {
    return 0;
  }
  const char *max_env = getenv("BUI_GPU_MAX_TEXTURE"); /* tests: force tiling */
  if (max_env && atoi(max_env) >= 64) {
    g->max_tex = atoi(max_env) < g->max_tex ? atoi(max_env) : g->max_tex;
  }
  const uint32_t white = 0xFFFFFFFFu;
  g->white = g->ops->tex_create(g, BUI_FMT_RGBA8, 1, 1, &white, 4, BUI_TEX_IMMUTABLE);
  if (!g->white) {
    return bui_gpu_fail("white texture", 0);
  }
  if (!atlas_create(g, &g->mask, 2048, 1)) {
    return bui_gpu_fail("atlas", 0);
  }
  return 1;
}

/* Everything on the device, the backend's own objects last. */
static void device_exit(BuiGpu *g)
{
  if (!g->dev) {
    return;
  }
  tex_drop(g, &g->mask.tex);
  tex_drop(g, &g->rgba.tex);
  for (int i = 0; i < BUI_GPU_BIG_MAX; i++) {
    tex_drop(g, &g->big[i].tex);
    tex_drop(g, &g->big[i].uv);
  }
  tex_drop(g, &g->curve_tex);
  tex_drop(g, &g->band_tex);
  for (int i = 0; i < g->n_dtex; i++) {
    tex_drop(g, &g->dtex[i].tex);
  }
  for (int i = 0; i < g->n_dfx; i++) {
    if (g->dfx[i].prog) {
      g->ops->fx_free(g, g->dfx[i].prog);
      g->dfx[i].prog = NULL;
    }
  }
  tex_drop(g, &g->white);
  g->ops->exit(g);
  g->dev = NULL;
}

static DWORD WINAPI device_thread(void *arg)
{
  BuiGpu *g = (BuiGpu *)arg;
  int ok = device_init(g);
  if (!ok && g->ops->api == BUI_GPU_API_D3D12) {
    device_exit(g);
    bui_gpu_fail("Direct3D 12 (falling back to Direct3D 11)", 0);
    g->ops = &bui_gpu_d3d11_ops;
    ok = device_init(g);
  }
  InterlockedExchange(&g->state, ok ? 1 : -1);
  return 0;
}

BuiGpu *bui_gpu_create(void *hwnd, int alpha)
{
  BuiGpu *g = (BuiGpu *)calloc(1, sizeof(BuiGpu));
  if (!g) {
    return NULL;
  }
  g->ops = BUI_gpu_api() == BUI_GPU_API_D3D12 ? &bui_gpu_d3d12_ops : &bui_gpu_d3d11_ops;
  g->hwnd = (HWND)hwnd;
  g->alpha = alpha;
  g->mask_map = (BuiGpuSlot *)calloc(MASK_MAP, sizeof(BuiGpuSlot));
  g->img_map = (BuiGpuSlot *)calloc(IMG_MAP, sizeof(BuiGpuSlot));
  g->thread = g->mask_map && g->img_map ? CreateThread(NULL, 0, device_thread, g, 0, NULL) : NULL;
  if (!g->thread) {
    bui_gpu_free(g);
    return NULL;
  }
  return g;
}

int bui_gpu_state(BuiGpu *g, int wait)
{
  if (wait && g->thread) {
    WaitForSingleObject(g->thread, INFINITE);
  }
  const LONG s = InterlockedCompareExchange(&g->state, 0, 0);
  if (s != 0 && g->thread) {
    WaitForSingleObject(g->thread, INFINITE);
    CloseHandle(g->thread);
    g->thread = NULL;
  }
  return (int)s;
}

BuiGpuApi bui_gpu_api_of(const BuiGpu *g)
{
  return g->ops->api;
}

void bui_gpu_free(BuiGpu *g)
{
  if (!g) {
    return;
  }
  if (g->thread) {
    WaitForSingleObject(g->thread, INFINITE);
    CloseHandle(g->thread);
  }
  device_exit(g);
  free(g->dtex);
  free(g->dfx);
  free(g->fx);
  free(g->fxf);
  free(g->inst);
  free(g->tv);
  free(g->mv);
  free(g->mi);
  free(g->b);
  free(g->mask_map);
  free(g->img_map);
  free(g->slug_map);
  free(g);
}

/** \} */

/* -------------------------------------------------------------------- */
/** \name Recording
 * \{ */

static void flush(BuiGpu *g);

static void big_sweep(BuiGpu *g);

void bui_gpu_begin(BuiGpu *g, int w, int h)
{
  g->w = w > 0 ? w : 1;
  g->h = h > 0 ? h : 1;
  g->frame++;
  big_sweep(g);
}

static BuiGpuBatch *batch_new(BuiGpu *g, BuiGpuBatchKind kind, int first)
{
  if (g->n_b == g->cap_b) {
    const int cap = g->cap_b ? g->cap_b * 2 : 64;
    BuiGpuBatch *nb = (BuiGpuBatch *)realloc(g->b, sizeof(BuiGpuBatch) * (size_t)cap);
    if (!nb) {
      return NULL;
    }
    g->b = nb;
    g->cap_b = cap;
  }
  BuiGpuBatch *b = &g->b[g->n_b++];
  memset(b, 0, sizeof(*b));
  b->kind = kind;
  b->first = first;
  b->img = -1;
  return b;
}

/* The batch to append to: the last one when it is of the same kind. */
static BuiGpuBatch *batch_for(BuiGpu *g, BuiGpuBatchKind kind, int first)
{
  if (kind < BUI_BATCH_MESH && g->n_b > 0 && g->b[g->n_b - 1].kind == kind) {
    return &g->b[g->n_b - 1];
  }
  return batch_new(g, kind, first);
}

/* -- textured meshes ----------------------------------------------------------- */

static void dtex_release(BuiGpu *g, BuiGpuDevTex *t)
{
  tex_drop(g, &t->tex);
  t->id = 0;
}

/* The format of a render target (BUI_TARGET_*). */
static BuiGpuFormat target_format(int target)
{
  switch (target - 1) {
    case BUI_TARGET_RGBA16F:
      return BUI_FMT_RGBA16F;
    case BUI_TARGET_RGBA32F:
      return BUI_FMT_RGBA32F;
    default:
      return BUI_FMT_RGBA8;
  }
}

/* Creates or refreshes the device copy of registry texture `id`. */
static void dtex_get(BuiGpu *g, int id)
{
  BuiTexView v;
  if (id <= 0 || !bui_texture_view(id, &v)) {
    return;
  }
  const int slot = (id & 0xFFFF) - 1;
  if (slot >= g->n_dtex) {
    const int n = slot + 16;
    BuiGpuDevTex *nd = (BuiGpuDevTex *)realloc(g->dtex, sizeof(BuiGpuDevTex) * (size_t)n);
    if (!nd) {
      return;
    }
    memset(nd + g->n_dtex, 0, sizeof(BuiGpuDevTex) * (size_t)(n - g->n_dtex));
    g->dtex = nd;
    g->n_dtex = n;
  }
  BuiGpuDevTex *t = &g->dtex[slot];
  if (t->id == id && t->gen == v.gen) {
    return;
  }
  if (t->id == id && t->w == v.w && t->h == v.h) {
    /* updated: draws recorded so far still need the old pixels */
    flush(g);
    int rects[32][4];
    int n = bui_texture_changes(id, t->gen, rects, 32);
    if (n < 0) {
      rects[0][0] = rects[0][1] = 0;
      rects[0][2] = v.w;
      rects[0][3] = v.h;
      n = 1;
    }
    for (int i = 0; i < n; i++) {
      const int *r = rects[i];
      g->ops->tex_update(g, t->tex, r[0], r[1], r[2], r[3],
                         v.rgba + ((size_t)r[1] * v.w + r[0]) * 4, v.w * 4);
    }
    t->gen = v.gen;
    return;
  }
  if (t->id) {
    flush(g);
    dtex_release(g, t);
  }
  /* a render target starts as zeros */
  t->tex = g->ops->tex_create(g, v.target ? target_format(v.target) : BUI_FMT_RGBA8, v.w, v.h,
                              v.target ? NULL : v.rgba, v.w * 4, v.target ? BUI_TEX_TARGET : 0);
  if (!t->tex) {
    return;
  }
  t->id = id;
  t->gen = v.gen;
  t->w = v.w;
  t->h = v.h;
  t->target = v.target;
}

const BuiGpuDevTex *bui_gpu_dtex(const BuiGpu *g, int id)
{
  const int slot = (id & 0xFFFF) - 1;
  return id > 0 && slot >= 0 && slot < g->n_dtex && g->dtex[slot].id == id ? &g->dtex[slot] : NULL;
}

BuiGpuTex *bui_gpu_dtex_tex(const BuiGpu *g, int id)
{
  const BuiGpuDevTex *t = bui_gpu_dtex(g, id);
  return t ? t->tex : g->white;
}

/* Frees device copies of textures freed in the registry. */
static void dtex_sweep(BuiGpu *g)
{
  const unsigned fg = bui_texture_free_gen();
  if (fg == g->dtex_free_gen) {
    return;
  }
  g->dtex_free_gen = fg;
  BuiTexView v;
  for (int i = 0; i < g->n_dtex; i++) {
    if (g->dtex[i].id && !bui_texture_view(g->dtex[i].id, &v)) {
      flush(g);
      dtex_release(g, &g->dtex[i]);
    }
  }
}

static int grow(void **p, int *cap, int need, size_t size)
{
  if (need <= *cap) {
    return 1;
  }
  int c = *cap ? *cap : 4096;
  while (c < need) {
    c *= 2;
  }
  void *np = realloc(*p, size * (size_t)c);
  if (!np) {
    return 0;
  }
  *p = np;
  *cap = c;
  return 1;
}

void bui_gpu_mesh(BuiGpu *g,
                  const BuiMeshVertex *vtx,
                  int n_vtx,
                  const void *idx,
                  int idx32,
                  int n_idx,
                  float ox,
                  float oy,
                  int tex,
                  int nearest,
                  const int clip[4])
{
  n_idx -= n_idx % 3;
  if (n_idx <= 0 || n_vtx <= 0) {
    return;
  }
  dtex_sweep(g);
  dtex_get(g, tex); /* may flush: before anything of this mesh is recorded */
  if (!grow((void **)&g->mv, &g->cap_mv, g->n_mv + n_vtx, sizeof(BuiMeshVertex)) ||
      !grow((void **)&g->mi, &g->cap_mi, g->n_mi + n_idx, sizeof(uint32_t)))
  {
    return;
  }
  BuiGpuBatch *b = NULL;
  if (g->n_b > 0) { /* same texture, sampler and clip: one draw */
    BuiGpuBatch *last = &g->b[g->n_b - 1];
    if (last->kind == BUI_BATCH_MESH && last->tex == tex && last->nearest == nearest &&
        memcmp(last->clip, clip, sizeof(int[4])) == 0)
    {
      b = last;
    }
  }
  if (!b) {
    b = batch_for(g, BUI_BATCH_MESH, g->n_mi);
    if (!b) {
      return;
    }
    b->tex = tex;
    b->nearest = nearest;
    memcpy(b->clip, clip, sizeof(int[4]));
  }
  const uint32_t base = (uint32_t)g->n_mv;
  for (int i = 0; i < n_vtx; i++) {
    BuiMeshVertex *v = &g->mv[g->n_mv++];
    *v = vtx[i];
    v->x += ox;
    v->y += oy;
  }
  const uint16_t *i16 = (const uint16_t *)idx;
  const uint32_t *i32 = (const uint32_t *)idx;
  uint32_t *out = g->mi + g->n_mi;
  int n = 0;
  for (int i = 0; i < n_idx; i += 3) {
    const uint32_t a = idx32 ? i32[i] : i16[i], c1 = idx32 ? i32[i + 1] : i16[i + 1],
                   c2 = idx32 ? i32[i + 2] : i16[i + 2];
    if (a >= (uint32_t)n_vtx || c1 >= (uint32_t)n_vtx || c2 >= (uint32_t)n_vtx) {
      continue;
    }
    out[n++] = base + a;
    out[n++] = base + c1;
    out[n++] = base + c2;
  }
  g->n_mi += n;
  b->count += n;
}

/* Appends an instance to quad batch `b`. */
static BuiGpuInst *inst_in(BuiGpu *g, BuiGpuBatch *b, int kind, int x0, int y0, int x1, int y1)
{
  if (g->n_inst == g->cap_inst) {
    const int cap = g->cap_inst ? g->cap_inst * 2 : 1024;
    BuiGpuInst *ni = (BuiGpuInst *)realloc(g->inst, sizeof(BuiGpuInst) * (size_t)cap);
    if (!ni) {
      return NULL;
    }
    g->inst = ni;
    g->cap_inst = cap;
  }
  b->count++;
  BuiGpuInst *in = &g->inst[g->n_inst++];
  memset(in, 0, sizeof(*in));
  in->bounds[0] = (float)x0;
  in->bounds[1] = (float)y0;
  in->bounds[2] = (float)x1;
  in->bounds[3] = (float)y1;
  in->p[0] = (float)kind;
  return in;
}

BuiGpuInst *bui_gpu_inst_image(BuiGpu *g, int tex, int x0, int y0, int x1, int y1)
{
  BuiGpuBatch *b = g->n_b > 0 ? &g->b[g->n_b - 1] : NULL;
  if (!b || b->kind != BUI_BATCH_QUADS || (b->img >= 0 && b->img != tex)) {
    b = batch_new(g, BUI_BATCH_QUADS, g->n_inst); /* another texture: another draw */
  }
  if (!b) {
    return NULL;
  }
  b->img = tex;
  return inst_in(g, b, BG_IMAGE, x0, y0, x1, y1);
}

BuiGpuInst *bui_gpu_inst(BuiGpu *g, int kind, int x0, int y0, int x1, int y1)
{
  BuiGpuBatch *b = batch_for(g, BUI_BATCH_QUADS, g->n_inst);
  return b ? inst_in(g, b, kind, x0, y0, x1, y1) : NULL;
}

const void *bui_gpu_batch_data(const BuiGpu *g, BuiGpuBatchKind k, int *r_count)
{
  *r_count = k == BUI_BATCH_QUADS ? g->n_inst : k == BUI_BATCH_TRIS ? g->n_tv : g->n_mv;
  return k == BUI_BATCH_QUADS ? (const void *)g->inst :
         k == BUI_BATCH_TRIS  ? (const void *)g->tv :
                                (const void *)g->mv;
}

BuiGpuTex *bui_gpu_batch_image(const BuiGpu *g, const BuiGpuBatch *b, BuiGpuTex **r_uv)
{
  const BuiGpuBig *big = b->img > 0 && g->big[b->img - 1].tex ? &g->big[b->img - 1] : NULL;
  *r_uv = big ? big->uv : NULL;
  return big ? big->tex : g->rgba.tex;
}

void bui_gpu_triangles(BuiGpu *g,
                       const float (*co)[2],
                       const float (*col)[4],
                       int count,
                       const int clip[4])
{
  count -= count % 3;
  if (count <= 0) {
    return;
  }
  if (g->n_tv + count > g->cap_tv) {
    int cap = g->cap_tv ? g->cap_tv : 1024;
    while (cap < g->n_tv + count) {
      cap *= 2;
    }
    BuiGpuTriV *nt = (BuiGpuTriV *)realloc(g->tv, sizeof(BuiGpuTriV) * (size_t)cap);
    if (!nt) {
      return;
    }
    g->tv = nt;
    g->cap_tv = cap;
  }
  BuiGpuBatch *b = batch_for(g, BUI_BATCH_TRIS, g->n_tv);
  if (!b) {
    return;
  }
  for (int i = 0; i < count; i++) {
    BuiGpuTriV *v = &g->tv[g->n_tv++];
    v->x = co[i][0];
    v->y = co[i][1];
    memcpy(v->c, col[i], sizeof(v->c));
    for (int k = 0; k < 4; k++) {
      v->clip[k] = (float)clip[k];
    }
  }
  b->count += count;
}

/* -- effects (application shaders) -------------------------------------------------- */

static void dfx_release(BuiGpu *g, BuiGpuDevFx *f)
{
  if (f->prog) {
    g->ops->fx_free(g, f->prog);
  }
  memset(f, 0, sizeof(*f));
}

/* The device's pipeline of effect `v`, compiled on first use. NULL: no room. */
static BuiGpuDevFx *dfx_get(BuiGpu *g, const BuiEffectView *v)
{
  const int slot = BUI_EFFECT_SLOT(v->id);
  if (slot >= g->n_dfx) {
    const int n = slot + 8;
    BuiGpuDevFx *nd = (BuiGpuDevFx *)realloc(g->dfx, sizeof(BuiGpuDevFx) * (size_t)n);
    if (!nd) {
      return NULL;
    }
    memset(nd + g->n_dfx, 0, sizeof(BuiGpuDevFx) * (size_t)(n - g->n_dfx));
    g->dfx = nd;
    g->n_dfx = n;
  }
  BuiGpuDevFx *f = &g->dfx[slot];
  if (f->id == v->id) {
    return f;
  }
  if (f->id) { /* a freed effect's slot, reused */
    flush(g);
    dfx_release(g, f);
  }
  char log[BUI_EFFECT_LOG] = "";
  f->id = v->id;
  f->n_inputs = v->n_inputs;
  f->n_floats = v->n_floats;
  f->prog = g->ops->fx_build(g, v, log, sizeof(log));
  if (!f->prog) {
    f->failed = 1;
    bui_gpu_fail(log, 0);
  }
  bui_effect_set_error(v->id, log);
  return f;
}

/* Frees the pipelines of effects freed in the registry. */
static void dfx_sweep(BuiGpu *g)
{
  const unsigned fg = bui_effect_free_gen();
  if (fg == g->dfx_free_gen) {
    return;
  }
  g->dfx_free_gen = fg;
  BuiEffectView v;
  for (int i = 0; i < g->n_dfx; i++) {
    if (g->dfx[i].id && !bui_effect_view(g->dfx[i].id, &v)) {
      flush(g);
      dfx_release(g, &g->dfx[i]);
    }
  }
}

const BuiGpuDevFx *bui_gpu_dfx(const BuiGpu *g, int effect)
{
  const int slot = BUI_EFFECT_SLOT(effect);
  const BuiGpuDevFx *f = slot >= 0 && slot < g->n_dfx ? &g->dfx[slot] : NULL;
  return f && f->id == effect && f->prog ? f : NULL;
}

size_t bui_gpu_fx_consts(const BuiGpu *g, const BuiGpuFxCall *fc, int n_floats, int tw, int th,
                         BuiGpuFxConsts *r_consts)
{
  float *data = r_consts->data;
  memcpy(data, fc->rect, sizeof(fc->rect));
  data[4] = 2.0f / (float)tw;
  data[5] = 2.0f / (float)th;
  data[6] = data[7] = 0.0f;
  for (int i = 0; i < n_floats; i++) { /* HLSL gives each element of an array a register */
    float *reg = data + 8 + 4 * i;
    reg[0] = g->fxf[fc->floats + i];
    reg[1] = reg[2] = reg[3] = 0.0f;
  }
  if (n_floats == 0) {
    memset(data + 8, 0, sizeof(float) * 4);
  }
  return sizeof(float) * (size_t)(8 + 4 * (n_floats > 0 ? n_floats : 1));
}

int bui_gpu_effect(BuiGpu *g,
                   int dst,
                   int effect,
                   const float rect[4],
                   const int inputs[BUI_EFFECT_MAX_INPUTS],
                   const float *floats,
                   int n_floats,
                   const int clip[4])
{
  BuiEffectView v;
  if (!bui_effect_view(effect, &v)) {
    return 0;
  }
  dfx_sweep(g);
  BuiGpuDevFx *f = dfx_get(g, &v);
  if (!f || f->failed) {
    return 0;
  }
  dtex_sweep(g);
  if (dst) {
    dtex_get(g, dst);
    const BuiGpuDevTex *d = bui_gpu_dtex(g, dst);
    if (!d || !d->target) {
      return 0; /* not a render target, or the device could not make it */
    }
    for (int k = 0; k < v.n_inputs; k++) {
      if (inputs[k] == dst) {
        return 0; /* a pass cannot sample the target it draws into */
      }
    }
  }
  for (int k = 0; k < v.n_inputs; k++) {
    dtex_get(g, inputs[k]); /* may flush: before anything of this draw is recorded */
  }
  if (!grow((void **)&g->fx, &g->cap_fx, g->n_fx + 1, sizeof(BuiGpuFxCall)) ||
      !grow((void **)&g->fxf, &g->cap_fxf, g->n_fxf + v.n_floats, sizeof(float)))
  {
    return 0;
  }
  /* never merged: each draw has its own shader state */
  BuiGpuBatch *b = batch_new(g, BUI_BATCH_FX, g->n_fx);
  if (!b) {
    return 0;
  }
  b->count = 1;
  BuiGpuFxCall *c = &g->fx[g->n_fx++];
  c->effect = effect;
  c->dst = dst;
  memcpy(c->rect, rect, sizeof(c->rect));
  memcpy(c->inputs, inputs, sizeof(c->inputs));
  memcpy(c->clip, clip, sizeof(c->clip));
  c->floats = g->n_fxf;
  const int n = floats && n_floats > 0 ? (n_floats < v.n_floats ? n_floats : v.n_floats) : 0;
  if (n > 0) {
    memcpy(g->fxf + g->n_fxf, floats, sizeof(float) * (size_t)n);
  }
  memset(g->fxf + g->n_fxf + n, 0, sizeof(float) * (size_t)(v.n_floats - n));
  g->n_fxf += v.n_floats;
  return 1;
}

int bui_gpu_target_clear(BuiGpu *g, int target, const float rgba[4])
{
  dtex_sweep(g);
  dtex_get(g, target);
  const BuiGpuDevTex *d = bui_gpu_dtex(g, target);
  if (!d || !d->target || !grow((void **)&g->fx, &g->cap_fx, g->n_fx + 1, sizeof(BuiGpuFxCall)) ||
      !grow((void **)&g->fxf, &g->cap_fxf, g->n_fxf + 4, sizeof(float)))
  {
    return 0;
  }
  BuiGpuBatch *b = batch_new(g, BUI_BATCH_FX, g->n_fx);
  if (!b) {
    return 0;
  }
  b->count = 1;
  BuiGpuFxCall *c = &g->fx[g->n_fx++];
  memset(c, 0, sizeof(*c));
  c->dst = target; /* effect 0: a clear */
  c->floats = g->n_fxf;
  memcpy(g->fxf + g->n_fxf, rgba, sizeof(float) * 4);
  g->n_fxf += 4;
  return 1;
}

static uint64_t mix64(uint64_t h)
{
  h ^= h >> 33;
  h *= 0xff51afd7ed558ccdull;
  h ^= h >> 33;
  return h;
}

static void mask_reset(BuiGpu *g)
{
  flush(g); /* recorded instances still point into the old layout */
  atlas_clear(&g->mask);
  memset(g->mask_map, 0, sizeof(BuiGpuSlot) * MASK_MAP);
  g->mask_n = 0;
}

int bui_gpu_mask(BuiGpu *g, const void *key, unsigned gen, const uint8_t *mask, int w, int h,
                 int *r_x, int *r_y)
{
  if (w + 1 > g->mask.size || h + 1 > g->mask.size) {
    return 0;
  }
  if (gen != g->mask_gen) { /* text runs were freed: their keys may be reused */
    g->mask_gen = gen;
    mask_reset(g);
  }
  const uint64_t k1 = key ? (uint64_t)(uintptr_t)key : 0, k2 = ((uint64_t)w << 32) | (uint32_t)h;
  uint32_t idx = 0;
  if (key) {
    idx = (uint32_t)mix64(k1) & (MASK_MAP - 1);
    for (;;) {
      BuiGpuSlot *s = &g->mask_map[idx];
      if (s->k1 == 0) {
        break;
      }
      if (s->k1 == k1 && s->k2 == k2) {
        *r_x = s->x;
        *r_y = s->y;
        return 1;
      }
      idx = (idx + 1) & (MASK_MAP - 1);
    }
    if (g->mask_n >= MASK_MAP_FILL) {
      mask_reset(g);
      return bui_gpu_mask(g, key, gen, mask, w, h, r_x, r_y);
    }
  }
  int x, y;
  if (!atlas_place(&g->mask, w, h, &x, &y)) {
    mask_reset(g); /* empty now, and the map slot must be found again */
    return bui_gpu_mask(g, key, gen, mask, w, h, r_x, r_y);
  }
  g->ops->tex_update(g, g->mask.tex, x, y, w, h, mask, w);
  if (key) {
    BuiGpuSlot *s = &g->mask_map[idx];
    s->k1 = k1;
    s->k2 = k2;
    s->x = x;
    s->y = y;
    g->mask_n++;
  }
  *r_x = x;
  *r_y = y;
  return 1;
}

/* -------------------------------------------------------------------- */
/** \name Slug glyphs
 *
 * Eric Lengyel's Slug algorithm (https://github.com/EricLengyel/Slug):
 * glyph outlines live on the GPU as quadratic curves, split into horizontal
 * and vertical bands; the pixel shader casts a ray each way through the
 * pixel's bands and integrates exact coverage. Resolution independent, one
 * quad per glyph, no rasterizing or atlas packing on the CPU.
 * \{ */

int bui_gpu_slug_on(BuiGpu *g)
{
  if (g->slug == 0) {
    g->slug = -1;
    g->curve_tex = g->ops->tex_create(g, BUI_FMT_RGBA32F, SLUG_W, SLUG_H, NULL, 0, 0);
    g->band_tex = g->curve_tex ? g->ops->tex_create(g, BUI_FMT_RG32UI, SLUG_W, SLUG_H, NULL, 0, 0) :
                                 NULL;
    if (g->band_tex && (g->slug_map = (BuiGpuSlugGlyph *)calloc(SLUG_MAP, sizeof(BuiGpuSlugGlyph))))
    {
      g->slug = 1;
    }
  }
  return g->slug > 0;
}

static void slug_reset(BuiGpu *g)
{
  flush(g); /* recorded instances still point at the old data */
  memset(g->slug_map, 0, sizeof(BuiGpuSlugGlyph) * SLUG_MAP);
  g->slug_n = g->curve_next = g->band_next = 0;
}

/* Uploads `count` texels at linear index `at`, row by row. */
static void slug_upload(BuiGpu *g, BuiGpuTex *tex, int at, const void *data, int count, int texel)
{
  const char *p = (const char *)data;
  while (count > 0) {
    const int x = at % SLUG_W, y = at / SLUG_W;
    const int n = count < SLUG_W - x ? count : SLUG_W - x;
    g->ops->tex_update(g, tex, x, y, n, 1, p, n * texel);
    p += (size_t)n * texel;
    at += n;
    count -= n;
  }
}

typedef struct SlugSort {
  float key;
  uint32_t curve;
} SlugSort;

static int slug_cmp(const void *a, const void *b)
{
  const float ka = ((const SlugSort *)a)->key, kb = ((const SlugSort *)b)->key;
  return ka < kb ? 1 : ka > kb ? -1 : 0; /* descending */
}

static float min3(float a, float b, float c)
{
  return a < b ? (a < c ? a : c) : (b < c ? b : c);
}
static float max3(float a, float b, float c)
{
  return a > b ? (a > c ? a : c) : (b > c ? b : c);
}

/* Builds and uploads a glyph's curve and band data. */
static int slug_build(BuiGpu *g, BuiGpuSlugGlyph *sg, const float *cv, int n, const float bb[4])
{
  const int nb = n < 8 ? 1 : (n / 4 < SLUG_MAX_BANDS ? n / 4 : SLUG_MAX_BANDS);
  const float w = bb[2] - bb[0] > 1.0f ? bb[2] - bb[0] : 1.0f;
  const float h = bb[3] - bb[1] > 1.0f ? bb[3] - bb[1] : 1.0f;
  /* horizontal bands split y, vertical bands split x */
  const float hs = (float)nb / h, vs = (float)nb / w;
  const float eps = 1.0f / 1024.0f * (w > h ? w : h);

  /* worst case every curve lands in every band */
  const int max_entries = 2 * nb * n;
  const int need_band = 2 * nb + max_entries, need_curve = 2 * n;
  if (need_curve > SLUG_W * SLUG_H / 4 || need_band > SLUG_W * SLUG_H / 4) {
    return 0; /* absurd glyph */
  }
  if (g->curve_next + need_curve > SLUG_W * SLUG_H || g->band_next + need_band > SLUG_W * SLUG_H ||
      g->slug_n >= SLUG_MAP_FILL)
  {
    return -1; /* full: caller resets and retries */
  }
  float *ct = (float *)malloc(sizeof(float) * 8 * (size_t)n);
  uint32_t *bt = (uint32_t *)malloc(sizeof(uint32_t) * 2 * (size_t)need_band);
  SlugSort *tmp = (SlugSort *)malloc(sizeof(SlugSort) * (size_t)n);
  if (!ct || !bt || !tmp) {
    free(ct);
    free(bt);
    free(tmp);
    return 0;
  }
  const int cbase = g->curve_next; /* even: a curve never straddles a row */
  for (int i = 0; i < n; i++) {
    const float *q = cv + 6 * i;
    float *t = ct + 8 * i;
    t[0] = q[0];
    t[1] = q[1];
    t[2] = q[2];
    t[3] = q[3];
    t[4] = q[4];
    t[5] = q[5];
    t[6] = t[7] = 0.0f;
  }
  const int bbase = g->band_next;
  int at = 2 * nb; /* entries follow the 2 * nb headers */
  for (int dir = 0; dir < 2; dir++) {
    for (int b = 0; b < nb; b++) {
      /* band range in font units */
      const float s = dir == 0 ? hs : vs, o = dir == 0 ? bb[1] : bb[0];
      const float lo = o + (float)b / s - eps, hi = o + (float)(b + 1) / s + eps;
      int m = 0;
      for (int i = 0; i < n; i++) {
        const float *q = cv + 6 * i;
        /* dir 0 (horizontal rays) uses y extents, sorted by max x; dir 1 the reverse */
        const float a0 = dir == 0 ? q[1] : q[0], a1 = dir == 0 ? q[3] : q[2],
                    a2 = dir == 0 ? q[5] : q[4];
        const float amin = min3(a0, a1, a2), amax = max3(a0, a1, a2);
        if (amax < lo || amin > hi || (a0 == a1 && a1 == a2)) {
          continue; /* outside, or parallel to the ray: never crosses it */
        }
        tmp[m].key = dir == 0 ? max3(q[0], q[2], q[4]) : max3(q[1], q[3], q[5]);
        tmp[m].curve = (uint32_t)(cbase + 2 * i);
        m++;
      }
      qsort(tmp, (size_t)m, sizeof(SlugSort), slug_cmp);
      const int hdr = dir * nb + b;
      bt[2 * hdr] = (uint32_t)m;
      bt[2 * hdr + 1] = (uint32_t)(bbase + at);
      for (int k = 0; k < m; k++) {
        bt[2 * (at + k)] = tmp[k].curve;
        bt[2 * (at + k) + 1] = 0;
      }
      at += m;
    }
  }
  slug_upload(g, g->curve_tex, cbase, ct, 2 * n, 16);
  slug_upload(g, g->band_tex, bbase, bt, at, 8);
  g->curve_next += 2 * n;
  g->band_next += at;
  free(ct);
  free(bt);
  free(tmp);
  memcpy(sg->bbox, bb, sizeof(float[4]));
  sg->band[0] = hs;
  sg->band[1] = -bb[1] * hs;
  sg->band[2] = vs;
  sg->band[3] = -bb[0] * vs;
  sg->loc[0] = bbase;
  sg->loc[1] = nb;
  sg->loc[2] = nb;
  sg->loc[3] = 0;
  sg->ok = 1;
  return 1;
}

int bui_gpu_slug_glyph(BuiGpu *g, int face, int glyph, unsigned gen, float r_bbox[4],
                       float r_band[4], int r_loc[4])
{
  if (g->slug <= 0 || face < 0 || face > 0x7ffe || glyph < 0 || glyph > 0xffff) {
    return 0;
  }
  if (gen != g->slug_gen) { /* faces were freed: ids may be reused */
    g->slug_gen = gen;
    slug_reset(g);
  }
  const uint32_t key = ((uint32_t)(face + 1) << 16) | (uint32_t)glyph;
  uint32_t idx = (key * 2654435761u) >> 19 & (SLUG_MAP - 1);
  BuiGpuSlugGlyph *sg;
  for (;;) {
    sg = &g->slug_map[idx];
    if (sg->key == 0 || sg->key == key) {
      break;
    }
    idx = (idx + 1) & (SLUG_MAP - 1);
  }
  if (sg->key == 0) {
    float *cv = NULL;
    int n = 0;
    float bb[4];
    sg->ok = 0;
    if (bui_text_glyph_outline(face, glyph, &cv, &n, bb)) {
      int got = slug_build(g, sg, cv, n, bb);
      if (got < 0) {
        slug_reset(g);
        free(cv);
        return bui_gpu_slug_glyph(g, face, glyph, gen, r_bbox, r_band, r_loc);
      }
    }
    free(cv);
    sg->key = key;
    g->slug_n++;
  }
  if (!sg->ok) {
    return 0;
  }
  memcpy(r_bbox, sg->bbox, sizeof(float[4]));
  memcpy(r_band, sg->band, sizeof(float[4]));
  memcpy(r_loc, sg->loc, sizeof(int[4]));
  return 1;
}

/** \} */

static void img_reset(BuiGpu *g)
{
  flush(g);
  atlas_clear(&g->rgba);
  memset(g->img_map, 0, sizeof(BuiGpuSlot) * IMG_MAP);
  g->img_n = 0;
}

int bui_gpu_image(BuiGpu *g, uint64_t key, const uint32_t *argb, int w, int h, int *r_x, int *r_y)
{
  if (!g->rgba.tex && !atlas_create(g, &g->rgba, 2048, 4)) {
    return 0;
  }
  if (w + 1 > g->rgba.size || h + 1 > g->rgba.size) {
    return 0; /* TODO: images larger than the atlas */
  }
  /* key: BuiPixmap.key or its content hash (gpu_raster.c), with the size */
  const uint64_t k1 = key, k2 = ((uint64_t)w << 32) | (uint32_t)h;
  uint32_t idx = (uint32_t)mix64(k1) & (IMG_MAP - 1);
  for (;;) {
    BuiGpuSlot *s = &g->img_map[idx];
    if (s->k1 == 0) {
      break;
    }
    if (s->k1 == k1 && s->k2 == k2) {
      *r_x = s->x;
      *r_y = s->y;
      return 1;
    }
    idx = (idx + 1) & (IMG_MAP - 1);
  }
  int x, y;
  if (g->img_n >= IMG_MAP_FILL || !atlas_place(&g->rgba, w, h, &x, &y)) {
    img_reset(g);
    return bui_gpu_image(g, key, argb, w, h, r_x, r_y);
  }
  g->ops->tex_update(g, g->rgba.tex, x, y, w, h, argb, w * 4);
  BuiGpuSlot *s = &g->img_map[idx];
  s->k1 = k1;
  s->k2 = k2;
  s->x = x;
  s->y = y;
  g->img_n++;
  *r_x = x;
  *r_y = y;
  return 1;
}

/* -- big images: textures of their own ------------------------------------------------ */

static size_t big_size(int format, int w, int h)
{
  return format == BUI_PIXMAP_ARGB ? (size_t)w * h * 4 : (size_t)w * h * 3 / 2;
}

static void big_release(BuiGpu *g, BuiGpuBig *t)
{
  if (t->tex) {
    g->big_bytes -= big_size(t->format, t->w, t->h);
  }
  tex_drop(g, &t->tex);
  tex_drop(g, &t->uv);
  memset(t, 0, sizeof(*t));
}

/* `px` into slot `t`'s textures: created, or (a stream's next frame) updated in place. */
static int big_upload(BuiGpu *g, BuiGpuBig *t, int create, const uint32_t *px, int stride)
{
  const int nv12 = t->format != BUI_PIXMAP_ARGB;
  const uint8_t *uv = (const uint8_t *)px + (size_t)t->w * t->h;
  if (!create) {
    g->ops->tex_update(g, t->tex, 0, 0, t->w, t->h, px, stride * (nv12 ? 1 : 4));
    if (nv12) {
      g->ops->tex_update(g, t->uv, 0, 0, t->w / 2, t->h / 2, uv, t->w);
    }
    return 1;
  }
  const int flags = t->stream ? 0 : BUI_TEX_IMMUTABLE;
  if (!nv12) { /* 0xAARRGGBB in memory order */
    t->tex = g->ops->tex_create(g, BUI_FMT_BGRA8, t->w, t->h, px, stride * 4, flags);
    return t->tex != NULL;
  }
  t->tex = g->ops->tex_create(g, BUI_FMT_R8, t->w, t->h, px, t->w, flags);
  t->uv = t->tex ? g->ops->tex_create(g, BUI_FMT_RG8, t->w / 2, t->h / 2, uv, t->w, flags) : NULL;
  if (!t->uv) {
    tex_drop(g, &t->tex);
    return 0;
  }
  return 1;
}

/* Frees what the kit freed (BUI_image_forget) and what went unused. */
static void big_sweep(BuiGpu *g)
{
  uint64_t gone[64];
  const int n = bui_image_forgotten(&g->forget_seen, gone, 64); /* -1: missed some */
  for (int i = 0; i < BUI_GPU_BIG_MAX; i++) {
    BuiGpuBig *t = &g->big[i];
    if (!t->key) {
      continue;
    }
    int drop = g->frame - t->used > BIG_IDLE || (n < 0 && t->used != g->frame);
    for (int k = 0; k < n && !drop; k++) {
      drop = gone[k] == t->base;
    }
    if (drop) {
      big_release(g, t);
    }
  }
}

int bui_gpu_max_texture(BuiGpu *g)
{
  return g->max_tex > 0 ? g->max_tex : 8192;
}

int bui_gpu_image_tex(BuiGpu *g, uint64_t key, uint64_t base, int stream, int format,
                      const uint32_t *px, int stride, int w, int h)
{
  if (w <= 0 || h <= 0 || w > bui_gpu_max_texture(g) || h > bui_gpu_max_texture(g)) {
    return 0;
  }
  int free_slot = -1, lru = -1;
  for (int i = 0; i < BUI_GPU_BIG_MAX; i++) {
    BuiGpuBig *t = &g->big[i];
    if (t->key && t->w == w && t->h == h && t->format == format &&
        (t->key == key || (stream && t->stream && t->base == base)))
    {
      if (t->key != key) { /* the stream's next frame: in place, no new texture */
        big_upload(g, t, 0, px, stride);
        t->key = key;
      }
      t->used = g->frame;
      return i + 1;
    }
    if (!t->key) {
      free_slot = free_slot < 0 ? i : free_slot;
    }
    else if (t->used != g->frame && (lru < 0 || t->used < g->big[lru].used)) {
      lru = i; /* not drawn this frame: no recorded batch samples it */
    }
  }
  const size_t bytes = big_size(format, w, h);
  while (lru >= 0 && (free_slot < 0 || g->big_bytes + bytes > BIG_BUDGET)) {
    big_release(g, &g->big[lru]);
    free_slot = free_slot < 0 ? lru : free_slot;
    lru = -1;
    for (int i = 0; i < BUI_GPU_BIG_MAX; i++) {
      const BuiGpuBig *t = &g->big[i];
      if (t->key && t->used != g->frame && (lru < 0 || t->used < g->big[lru].used)) {
        lru = i;
      }
    }
  }
  if (free_slot < 0) {
    return 0;
  }
  BuiGpuBig *t = &g->big[free_slot];
  t->w = w;
  t->h = h;
  t->format = format;
  t->stream = stream;
  if (!big_upload(g, t, 1, px, stride)) {
    memset(t, 0, sizeof(*t));
    return 0;
  }
  t->key = key;
  t->base = base;
  t->used = g->frame;
  g->big_bytes += bytes;
  return free_slot + 1;
}

void bui_gpu_damage(BuiGpu *g, int x, int y, int w, int h)
{
  if (w <= 0 || h <= 0 || g->n_dmg == (int)(sizeof(g->dmg) / sizeof(g->dmg[0]))) {
    g->dmg_full = 1;
    return;
  }
  RECT *r = &g->dmg[g->n_dmg++];
  r->left = x;
  r->top = y;
  r->right = x + w;
  r->bottom = y + h;
}

/** \} */

/* -------------------------------------------------------------------- */
/** \name Drawing + present
 * \{ */

/* Resizing steps: the target and the swap chain are made a multiple of GPU_SLACK px wide and
 * high and kept while the frame stays within [cap - 2 * GPU_SLACK, cap], so a drag-resize
 * reallocates every few dozen sizes, not at each one. An alpha window's swap chain is shown at
 * the window's size, so it stays exact. */
#  define GPU_SLACK 256

static int slack_cap(const BuiGpu *g, int need)
{
  if (g->alpha) {
    return need;
  }
  const int cap = (need + GPU_SLACK - 1) / GPU_SLACK * GPU_SLACK;
  return cap < g->max_tex ? cap : need;
}

static int slack_fits(const BuiGpu *g, int cap, int need)
{
  return cap >= need && (g->alpha ? cap == need : cap - need < 2 * GPU_SLACK);
}

static int ensure_target(BuiGpu *g)
{
  if (g->has_target && slack_fits(g, g->tw, g->w) && slack_fits(g, g->th, g->h)) {
    if (g->fw != g->w || g->fh != g->h) { /* another size in the same texture: a redraw follows */
      g->fw = g->w;
      g->fh = g->h;
      g->dmg_full = 1;
      return g->ops->target(g, g->tw, g->th); /* the same size: cleared */
    }
    return 1;
  }
  const int tw = slack_cap(g, g->w), th = slack_cap(g, g->h);
  g->has_target = g->ops->target(g, tw, th);
  if (!g->has_target) {
    return 0;
  }
  g->tw = tw;
  g->th = th;
  g->fw = g->w;
  g->fh = g->h;
  g->dmg_full = 1;
  return 1;
}

static void flush(BuiGpu *g)
{
  if (g->n_b == 0) {
    return;
  }
  if (ensure_target(g)) {
    g->ops->draw(g);
  }
  g->n_b = g->n_inst = g->n_tv = g->n_mv = g->n_mi = g->n_fx = g->n_fxf = 0;
}

static int ensure_swap(BuiGpu *g)
{
  if (!g->hwnd) { /* offscreen */
    return 0;
  }
  if (g->has_swap && slack_fits(g, g->sw, g->w) && slack_fits(g, g->sh, g->h)) {
    return 1;
  }
  const int cw = slack_cap(g, g->w), ch = slack_cap(g, g->h);
  g->has_swap = g->ops->swap(g, cw, ch);
  if (!g->has_swap) {
    return 0;
  }
  g->sw = cw;
  g->sh = ch;
  g->swap_fresh = 1;
  return 1;
}

void bui_gpu_clear(BuiGpu *g, int x, int y, int w, int h)
{
  flush(g); /* what was recorded for earlier regions comes first */
  if (!g->alpha || !g->has_target || g->fw != g->w || g->fh != g->h) {
    return; /* a new target starts transparent */
  }
  const RECT rect = {x, y, x + w, y + h};
  g->ops->clear(g, &rect);
}

int bui_gpu_present(BuiGpu *g, int full)
{
  flush(g);
  if (!g->has_target || !ensure_swap(g)) {
    g->n_dmg = g->dmg_full = 0;
    if (g->ops->submit) {
      g->ops->submit(g);
    }
    return g->has_target && !g->alpha; /* no alpha surface: the renderer is given up */
  }
  for (int i = 0; i < g->n_dmg; i++) { /* clamp: DXGI rejects rects outside the buffer */
    RECT *r = &g->dmg[i];
    r->left = r->left < 0 ? 0 : r->left;
    r->top = r->top < 0 ? 0 : r->top;
    r->right = r->right > g->w ? g->w : r->right;
    r->bottom = r->bottom > g->h ? g->h : r->bottom;
    if (r->right <= r->left || r->bottom <= r->top) {
      *r = g->dmg[--g->n_dmg];
      i--;
    }
  }
  const int whole = full || g->dmg_full || g->swap_fresh || g->n_dmg == 0 || g->alpha;
  /* copying rects adding up to half the frame or more: one copy of it all is cheaper */
  long area = 0;
  for (int k = 0; k < 2 && !whole && !g->prev_whole; k++) {
    const RECT *rs = k ? g->prev : g->dmg;
    for (int i = 0, n = k ? g->n_prev : g->n_dmg; i < n; i++) {
      area += (long)(rs[i].right - rs[i].left) * (rs[i].bottom - rs[i].top);
    }
  }
  /* the buffer is two frames old (or of the bitblt model): the whole frame; else what changed
   * this frame and the last one, the rest is already there */
  const int copy_whole = whole || g->prev_whole || area * 2 >= (long)g->w * g->h;
  RECT copy[32];
  int n_copy = 0;
  if (!copy_whole) {
    memcpy(copy, g->dmg, sizeof(RECT) * (size_t)g->n_dmg);
    memcpy(copy + g->n_dmg, g->prev, sizeof(RECT) * (size_t)g->n_prev);
    n_copy = g->n_dmg + g->n_prev;
  }
  g->prev_whole = whole;
  g->n_prev = whole ? 0 : g->n_dmg;
  memcpy(g->prev, g->dmg, sizeof(RECT) * (size_t)g->n_prev);
  const int dirty = !(full || g->dmg_full || g->swap_fresh) && g->n_dmg > 0;
  const int st = g->ops->present(g, copy_whole, copy, n_copy, g->dmg, dirty ? g->n_dmg : 0);
  g->swap_fresh = st == BUI_PRESENT_OCCLUDED;
  g->n_dmg = g->dmg_full = 0;
  return st != BUI_PRESENT_LOST;
}

int bui_gpu_read(BuiGpu *g, uint32_t *dst, int w, int h)
{
  flush(g);
  if (!g->has_target || g->fw != w || g->fh != h || !g->ops->read(g, dst, w, h)) {
    return 0;
  }
  /* like the raster: 0x00RRGGBB, or premultiplied 0xAARRGGBB with alpha */
  if (!g->alpha) {
    for (size_t i = 0, n = (size_t)w * h; i < n; i++) {
      dst[i] &= 0xFFFFFFu;
    }
  }
  return 1;
}

/** \} */

#else /* no GPU renderer on Windows here (built without BUI_GPU; Linux has gpu_gl.c): every
       * raster stays in software */

#  include "gpu_d3d11.h"

#  if !defined(BUI_GPU_GL)

#    include <stddef.h>

int bui_gpu_slug_on(BuiGpu *g)
{
  (void)g;
  return 0;
}
int bui_gpu_slug_glyph(BuiGpu *g, int face, int glyph, unsigned gen, float r_bbox[4],
                       float r_band[4], int r_loc[4])
{
  (void)g;
  (void)face;
  (void)glyph;
  (void)gen;
  (void)r_bbox;
  (void)r_band;
  (void)r_loc;
  return 0;
}
BuiGpu *bui_gpu_create(void *hwnd, int alpha)
{
  (void)hwnd;
  (void)alpha;
  return NULL;
}
void bui_gpu_free(BuiGpu *g)
{
  (void)g;
}
int bui_gpu_state(BuiGpu *g, int wait)
{
  (void)g;
  (void)wait;
  return -1;
}
BuiGpuApi bui_gpu_api_of(const BuiGpu *g)
{
  (void)g;
  return BUI_GPU_API_DEFAULT;
}
void bui_gpu_begin(BuiGpu *g, int w, int h)
{
  (void)g;
  (void)w;
  (void)h;
}
int bui_gpu_image_tex(BuiGpu *g, uint64_t key, uint64_t base, int stream, int format,
                      const uint32_t *argb, int stride, int w, int h)
{
  (void)g;
  (void)key;
  (void)base;
  (void)stream;
  (void)format;
  (void)argb;
  (void)stride;
  (void)w;
  (void)h;
  return 0;
}
int bui_gpu_max_texture(BuiGpu *g)
{
  (void)g;
  return 0;
}
BuiGpuInst *bui_gpu_inst_image(BuiGpu *g, int tex, int x0, int y0, int x1, int y1)
{
  (void)g;
  (void)tex;
  (void)x0;
  (void)y0;
  (void)x1;
  (void)y1;
  return NULL;
}
BuiGpuInst *bui_gpu_inst(BuiGpu *g, int kind, int x0, int y0, int x1, int y1)
{
  (void)g;
  (void)kind;
  (void)x0;
  (void)y0;
  (void)x1;
  (void)y1;
  return NULL;
}
int bui_gpu_mask(BuiGpu *g, const void *key, unsigned gen, const uint8_t *mask, int w, int h,
                 int *r_x, int *r_y)
{
  (void)g;
  (void)key;
  (void)gen;
  (void)mask;
  (void)w;
  (void)h;
  (void)r_x;
  (void)r_y;
  return 0;
}
int bui_gpu_image(BuiGpu *g, uint64_t key, const uint32_t *argb, int w, int h, int *r_x, int *r_y)
{
  (void)g;
  (void)key;
  (void)argb;
  (void)w;
  (void)h;
  (void)r_x;
  (void)r_y;
  return 0;
}
void bui_gpu_triangles(
    BuiGpu *g, const float (*co)[2], const float (*col)[4], int count, const int clip[4])
{
  (void)g;
  (void)co;
  (void)col;
  (void)count;
  (void)clip;
}
void bui_gpu_mesh(BuiGpu *g,
                  const struct BuiMeshVertex *vtx,
                  int n_vtx,
                  const void *idx,
                  int idx32,
                  int n_idx,
                  float ox,
                  float oy,
                  int tex,
                  int nearest,
                  const int clip[4])
{
  (void)g;
  (void)vtx;
  (void)n_vtx;
  (void)idx;
  (void)idx32;
  (void)n_idx;
  (void)ox;
  (void)oy;
  (void)tex;
  (void)nearest;
  (void)clip;
}
int bui_gpu_effect(BuiGpu *g,
                   int dst,
                   int effect,
                   const float rect[4],
                   const int inputs[BUI_EFFECT_MAX_INPUTS],
                   const float *floats,
                   int n_floats,
                   const int clip[4])
{
  (void)g;
  (void)dst;
  (void)effect;
  (void)rect;
  (void)inputs;
  (void)floats;
  (void)n_floats;
  (void)clip;
  return 0;
}
int bui_gpu_target_clear(BuiGpu *g, int target, const float rgba[4])
{
  (void)g;
  (void)target;
  (void)rgba;
  return 0;
}
void bui_gpu_clear(BuiGpu *g, int x, int y, int w, int h)
{
  (void)g;
  (void)x;
  (void)y;
  (void)w;
  (void)h;
}
void bui_gpu_damage(BuiGpu *g, int x, int y, int w, int h)
{
  (void)g;
  (void)x;
  (void)y;
  (void)w;
  (void)h;
}
int bui_gpu_present(BuiGpu *g, int full)
{
  (void)g;
  (void)full;
  return 0;
}
int bui_gpu_read(BuiGpu *g, uint32_t *dst, int w, int h)
{
  (void)g;
  (void)dst;
  (void)w;
  (void)h;
  return 0;
}

#  endif /* !BUI_GPU_GL */

#endif /* _WIN32 && BUI_GPU */
