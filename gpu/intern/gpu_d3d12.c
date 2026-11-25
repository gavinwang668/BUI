/* BUI GPU renderer: the Direct3D 12 backend of gpu_device.c (opt-in: BUI_gpu_set_api,
 * BUI_GPU_API; gpu_device.c falls back to Direct3D 11 when this one cannot start).
 *
 * The same pipeline as gpu_d3d11.c, drawn from the same shader bytecode (bui_ui.hlsl, shader model
 * 4.0, which Direct3D 12 takes with a root signature):
 *  - one root signature for every program: b0 the frame's constants (root constants), b1 an
 *    effect's constants (a root CBV), t0..t5 the renderer's textures and t8.. an effect's inputs
 *    (two descriptor tables), s0 (a sampler table: linear or point);
 *  - one command list, recorded across a frame and executed at present (or read); FRAMES frame
 *    contexts, each with its command allocator, its share of the upload ring (instances, vertices,
 *    constants and texture uploads are written into persistently mapped upload buffers), its
 *    shader-visible descriptor heap (tables are copied in from CPU heaps where every texture keeps
 *    its views) and the objects freed while it was recorded; a fence says when a context is free
 *    again, so the CPU runs at most FRAMES frames ahead of the GPU;
 *  - textures track their state; uploads leave them in COPY_DEST and the next draw moves them all
 *    back to PIXEL_SHADER_RESOURCE in one barrier;
 *  - a flip-model swap chain on the window (opaque) or, for alpha windows (flip-model swap chains
 *    on a window ignore alpha), a premultiplied composition swap chain shown by DirectComposition;
 *    the damaged rects of the persistent target are copied into it, as on Direct3D 11.
 * BUI_GPU_DEBUG=2 turns on the debug layer and prints its warnings and errors.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */

#if defined(_WIN32) && defined(BUI_GPU) /* BUI_GPU: features.json */

#  define COBJMACROS
#  include "gpu_dx.h"

#  include <d3d12.h>
#  include <d3d12sdklayers.h>

#  include <stdio.h>
#  include <stdlib.h>
#  include <string.h>

static const GUID k_IID_ID3D12Device = {
    0x189819f1, 0x1db6, 0x4b57, {0xbe, 0x54, 0x18, 0x21, 0x33, 0x9b, 0x85, 0xf7}};
static const GUID k_IID_ID3D12CommandQueue = {
    0x0ec870a6, 0x5d7e, 0x4c22, {0x8c, 0xfc, 0x5b, 0xaa, 0xe0, 0x76, 0x16, 0xed}};
static const GUID k_IID_ID3D12CommandAllocator = {
    0x6102dee4, 0xaf59, 0x4b09, {0xb9, 0x99, 0xb4, 0x4d, 0x73, 0xf0, 0x9b, 0x24}};
static const GUID k_IID_ID3D12GraphicsCommandList = {
    0x5b160d0f, 0xac1b, 0x4185, {0x8b, 0xa8, 0xb3, 0xae, 0x42, 0xa5, 0xa4, 0x55}};
static const GUID k_IID_ID3D12Fence = {
    0x0a753dcf, 0xc4d8, 0x4b91, {0xad, 0xf6, 0xbe, 0x5a, 0x60, 0xd9, 0x5a, 0x76}};
static const GUID k_IID_ID3D12DescriptorHeap = {
    0x8efb471d, 0x616c, 0x4f49, {0x90, 0xf7, 0x12, 0x7b, 0xb7, 0x63, 0xfa, 0x51}};
static const GUID k_IID_ID3D12Resource = {
    0x696442be, 0xa72e, 0x4059, {0xbc, 0x79, 0x5b, 0x5c, 0x98, 0x04, 0x0f, 0xad}};
static const GUID k_IID_ID3D12RootSignature = {
    0xc54a6b66, 0x72df, 0x4ee8, {0x8b, 0xe5, 0xa9, 0x46, 0xa1, 0x42, 0x92, 0x14}};
static const GUID k_IID_ID3D12PipelineState = {
    0x765a30f3, 0xf624, 0x4c6f, {0xa8, 0x28, 0xac, 0xe9, 0x48, 0x62, 0x24, 0x45}};
static const GUID k_IID_ID3D12Debug = {
    0x344488b7, 0x6846, 0x474b, {0xb9, 0x89, 0xf0, 0x27, 0x44, 0x82, 0x45, 0xe0}};
static const GUID k_IID_ID3D12InfoQueue = {
    0x0742a90b, 0xc387, 0x483f, {0xb9, 0x46, 0x30, 0xa7, 0xe4, 0xe6, 0x14, 0x58}};
static const GUID k_IID_ID3D12DebugDevice = {
    0x3febd6dd, 0x4973, 0x4787, {0x81, 0x94, 0xe4, 0x5f, 0x9e, 0x28, 0x92, 0x3e}};
static const GUID k_IID_IDXGISwapChain3 = {
    0x94d99bdb, 0xf1f8, 0x4ab0, {0xb2, 0x36, 0x7d, 0xa0, 0x17, 0x0e, 0xda, 0xb1}};
static const GUID k_IID_IDCompositionDevice = {
    0xc37ea93a, 0xe7aa, 0x450d, {0xb1, 0x6f, 0x97, 0x46, 0xcb, 0x04, 0x07, 0xf3}};

#  define FRAMES 2                         /* frame contexts: the CPU runs this many ahead */
#  define UPLOAD_CHUNK ((size_t)4 << 20)   /* upload buffers are made this big (or as needed) */
#  define VIEWS 4096                       /* shader-visible descriptors per frame context */
#  define HEAP_BLOCK 1024                  /* CPU descriptors per heap block */
#  define HEAP_BLOCKS 64
#  define TEX_TABLE 6                      /* t0..t5 */
#  define FX_TABLE BUI_EFFECT_MAX_INPUTS   /* t8.. */
#  define PEND_MAX 64

enum { ROOT_FRAME = 0, ROOT_FX_CB, ROOT_TEX, ROOT_FX_TEX, ROOT_SAMPLER };

struct BuiGpuTex {
  ID3D12Resource *res;
  D3D12_RESOURCE_STATES state;
  DXGI_FORMAT fmt;
  int texel;    /* bytes */
  int srv, rtv; /* CPU descriptor slots (rtv: -1 unless BUI_TEX_TARGET) */
  int pending;  /* in D12.pend: back to PIXEL_SHADER_RESOURCE before the next draw */
};

/* Effect pipelines by what they draw into: the frame (blended), then render targets of
 * BUI_TARGET_RGBA8, _RGBA16F, _RGBA32F (replaced, as on Direct3D 11). */
#  define FX_PSOS 4
struct BuiGpuProg {
  ID3DBlob *vs, *ps;
  ID3D12PipelineState *pso[FX_PSOS]; /* made on first use */
};

typedef struct Chunk {
  ID3D12Resource *buf;
  uint8_t *cpu;
  D3D12_GPU_VIRTUAL_ADDRESS gpu;
  size_t size, used;
} Chunk;

typedef struct Frame {
  ID3D12CommandAllocator *alloc;
  UINT64 fence; /* its work is done once the fence reaches this */
  Chunk *chunks;
  int n_chunks, at_chunk;
  ID3D12DescriptorHeap *views; /* shader visible */
  D3D12_CPU_DESCRIPTOR_HANDLE views_cpu;
  D3D12_GPU_DESCRIPTOR_HANDLE views_gpu;
  int n_views, cap_views;
  IUnknown **dead; /* released once its work is done */
  int n_dead, cap_dead;
} Frame;

/* A CPU descriptor heap: the views every texture keeps, copied into tables when drawn. */
typedef struct CpuHeap {
  D3D12_DESCRIPTOR_HEAP_TYPE type;
  ID3D12DescriptorHeap *block[HEAP_BLOCKS];
  D3D12_CPU_DESCRIPTOR_HANDLE start[HEAP_BLOCKS];
  int n_block, next;
  int *free_slots;
  int n_free;
  UINT inc;
} CpuHeap;

/* An upload ring allocation. */
typedef struct Upload {
  ID3D12Resource *buf;
  size_t offset;
  uint8_t *cpu;
  D3D12_GPU_VIRTUAL_ADDRESS gpu;
} Upload;

/* DirectComposition (alpha windows). dcomp.h declares its interfaces for C++ only (overloads), so
 * the few methods used here are declared by their vtable slots (as in dcomp.h's order). */
typedef struct DcUnknown DcUnknown;
typedef struct DcDeviceVtbl {
  HRESULT(STDMETHODCALLTYPE *QueryInterface)(DcUnknown *, REFIID, void **);
  ULONG(STDMETHODCALLTYPE *AddRef)(DcUnknown *);
  ULONG(STDMETHODCALLTYPE *Release)(DcUnknown *);
  HRESULT(STDMETHODCALLTYPE *Commit)(DcUnknown *);
  void *WaitForCommitCompletion, *GetFrameStatistics;
  HRESULT(STDMETHODCALLTYPE *CreateTargetForHwnd)(DcUnknown *, HWND, BOOL, DcUnknown **);
  HRESULT(STDMETHODCALLTYPE *CreateVisual)(DcUnknown *, DcUnknown **);
} DcDeviceVtbl;
typedef struct DcTargetVtbl {
  void *QueryInterface, *AddRef, *Release;
  HRESULT(STDMETHODCALLTYPE *SetRoot)(DcUnknown *, DcUnknown *);
} DcTargetVtbl;
typedef struct DcVisualVtbl {
  /* IUnknown, then SetOffsetX x2, SetOffsetY x2, SetTransform x2, SetTransformParent, SetEffect,
   * SetBitmapInterpolationMode, SetBorderMode, SetClip x2 */
  void *slots[15];
  HRESULT(STDMETHODCALLTYPE *SetContent)(DcUnknown *, IUnknown *);
} DcVisualVtbl;
struct DcUnknown {
  const void *vtbl;
};

typedef struct D12 {
  ID3D12Device *dev;
  ID3D12CommandQueue *queue;
  ID3D12GraphicsCommandList *list;
  ID3D12Fence *fence;
  HANDLE event;
  UINT64 fence_value;
  Frame frame[FRAMES];
  int cur;
  int open; /* the list is recording */
  int lost; /* the device was removed */

  ID3D12RootSignature *root;
  ID3D12PipelineState *pso[BUI_DX_PROGRAMS];
  ID3D12DescriptorHeap *samplers; /* shader visible: linear, point */
  D3D12_GPU_DESCRIPTOR_HANDLE sampler_gpu[2];
  UINT view_inc;
  CpuHeap srv_heap, rtv_heap;
  int null_srv;

  BuiGpuTex *pend[PEND_MAX];
  int n_pend;

  IDXGIFactory4 *factory;
  IDXGISwapChain3 *swap;
  ID3D12Resource *back[2];
  DcUnknown *dcomp, *dcomp_target, *dcomp_visual;

  BuiGpuTex *target;
  int tw, th;

  ID3D12InfoQueue *info; /* BUI_GPU_DEBUG=2 */
  int n_errors, n_warnings;
} D12;

#  define DEV(g) ((D12 *)(g)->dev)

/* ID3D12DescriptorHeap::GetCPU/GPUDescriptorHandleForHeapStart return a struct, which a C++ member
 * function returns through a hidden pointer after `this`; the C declarations of older SDKs say
 * otherwise. Called through the vtable with the real signature, they work with every header. */
typedef void(STDMETHODCALLTYPE *CpuStartFn)(ID3D12DescriptorHeap *, D3D12_CPU_DESCRIPTOR_HANDLE *);
typedef void(STDMETHODCALLTYPE *GpuStartFn)(ID3D12DescriptorHeap *, D3D12_GPU_DESCRIPTOR_HANDLE *);

static D3D12_CPU_DESCRIPTOR_HANDLE cpu_start(ID3D12DescriptorHeap *h)
{
  D3D12_CPU_DESCRIPTOR_HANDLE r;
  ((CpuStartFn)(void *)h->lpVtbl->GetCPUDescriptorHandleForHeapStart)(h, &r);
  return r;
}

static D3D12_GPU_DESCRIPTOR_HANDLE gpu_start(ID3D12DescriptorHeap *h)
{
  D3D12_GPU_DESCRIPTOR_HANDLE r;
  ((GpuStartFn)(void *)h->lpVtbl->GetGPUDescriptorHandleForHeapStart)(h, &r);
  return r;
}

static DXGI_FORMAT dxgi_format(BuiGpuFormat fmt, int *r_texel)
{
  static const DXGI_FORMAT map[] = {
      DXGI_FORMAT_R8_UNORM,           DXGI_FORMAT_R8G8_UNORM,         DXGI_FORMAT_B8G8R8A8_UNORM,
      DXGI_FORMAT_R8G8B8A8_UNORM,     DXGI_FORMAT_R16G16B16A16_FLOAT, DXGI_FORMAT_R32G32B32A32_FLOAT,
      DXGI_FORMAT_R32G32_UINT,
  };
  static const int texel[] = {1, 2, 4, 4, 8, 16, 8};
  *r_texel = texel[fmt];
  return map[fmt];
}

/* -------------------------------------------------------------------- */
/** \name Debug layer (BUI_GPU_DEBUG=2)
 * \{ */

/* Prints the warnings and errors the debug layer stored since the last call. `live`: they are
 * ReportLiveDeviceObjects' list, where anything but the device itself is a leak. */
static void debug_drain(D12 *d, int live)
{
  if (!d->info) {
    return;
  }
  const UINT64 n = ID3D12InfoQueue_GetNumStoredMessages(d->info);
  for (UINT64 i = 0; i < n; i++) {
    SIZE_T len = 0;
    if (FAILED(ID3D12InfoQueue_GetMessage(d->info, i, NULL, &len))) {
      continue;
    }
    D3D12_MESSAGE *m = (D3D12_MESSAGE *)malloc(len);
    if (m && SUCCEEDED(ID3D12InfoQueue_GetMessage(d->info, i, m, &len)) &&
        !(live && strncmp(m->pDescription, "Live ID3D12Device ", 18) == 0))
    {
      const int err = live || m->Severity <= D3D12_MESSAGE_SEVERITY_ERROR;
      d->n_errors += err;
      d->n_warnings += !err;
      fprintf(stderr, "bui gpu: d3d12 %s: %s\n", live ? "leak" : err ? "error" : "warning",
              m->pDescription);
    }
    free(m);
  }
  ID3D12InfoQueue_ClearStoredMessages(d->info);
}

/** \} */

/* -------------------------------------------------------------------- */
/** \name Frame contexts, the upload ring and descriptors
 * \{ */

static void wait_fence(D12 *d, UINT64 value)
{
  /* a removed device completes every value (UINT64_MAX): nothing waits forever */
  if (ID3D12Fence_GetCompletedValue(d->fence) < value &&
      SUCCEEDED(ID3D12Fence_SetEventOnCompletion(d->fence, value, d->event)))
  {
    WaitForSingleObject(d->event, INFINITE);
  }
}

static void gpu_idle(D12 *d);

/* Releases `obj` once the work recorded so far is done. */
static void defer_release(D12 *d, IUnknown *obj)
{
  if (!obj) {
    return;
  }
  Frame *f = &d->frame[d->cur];
  if (f->n_dead == f->cap_dead) {
    const int cap = f->cap_dead ? f->cap_dead * 2 : 64;
    IUnknown **nd = (IUnknown **)realloc(f->dead, sizeof(IUnknown *) * (size_t)cap);
    if (!nd) { /* out of memory: everything recorded runs first */
      gpu_idle(d);
      IUnknown_Release(obj);
      return;
    }
    f->dead = nd;
    f->cap_dead = cap;
  }
  f->dead[f->n_dead++] = obj;
}

static void frame_release(Frame *f)
{
  for (int i = 0; i < f->n_dead; i++) {
    IUnknown_Release(f->dead[i]);
  }
  f->n_dead = 0;
}

/* Starts recording into the current frame context once the GPU is done with it. */
static void ctx_begin(D12 *d)
{
  Frame *f = &d->frame[d->cur];
  wait_fence(d, f->fence);
  frame_release(f);
  for (int i = 0; i < f->n_chunks; i++) {
    f->chunks[i].used = 0;
  }
  f->at_chunk = 0;
  f->n_views = 0;
  if (FAILED(ID3D12CommandAllocator_Reset(f->alloc)) ||
      FAILED(ID3D12GraphicsCommandList_Reset(d->list, f->alloc, NULL)))
  {
    d->lost = 1;
    return;
  }
  d->open = 1;
  ID3D12DescriptorHeap *heaps[2] = {f->views, d->samplers};
  ID3D12GraphicsCommandList_SetDescriptorHeaps(d->list, 2, heaps);
  ID3D12GraphicsCommandList_SetGraphicsRootSignature(d->list, d->root);
}

/* Executes what was recorded and moves to the next frame context (ctx_begin starts it). */
static void ctx_submit(D12 *d)
{
  if (d->open) {
    d->open = 0;
    if (SUCCEEDED(ID3D12GraphicsCommandList_Close(d->list))) {
      ID3D12CommandList *lists[1] = {(ID3D12CommandList *)d->list};
      ID3D12CommandQueue_ExecuteCommandLists(d->queue, 1, lists);
    }
  }
  ID3D12CommandQueue_Signal(d->queue, d->fence, ++d->fence_value);
  d->frame[d->cur].fence = d->fence_value;
  d->cur = (d->cur + 1) % FRAMES;
  if (FAILED(ID3D12Device_GetDeviceRemovedReason(d->dev))) {
    d->lost = 1;
  }
  debug_drain(d, 0);
}

/* Everything recorded runs to completion; recording goes on in a fresh context. */
static void gpu_idle(D12 *d)
{
  ctx_submit(d);
  wait_fence(d, d->fence_value);
  ctx_begin(d);
}

static ID3D12Resource *buffer(D12 *d, D3D12_HEAP_TYPE heap, size_t size, D3D12_RESOURCE_STATES state)
{
  D3D12_HEAP_PROPERTIES hp = {heap, D3D12_CPU_PAGE_PROPERTY_UNKNOWN, D3D12_MEMORY_POOL_UNKNOWN, 1, 1};
  D3D12_RESOURCE_DESC rd;
  memset(&rd, 0, sizeof(rd));
  rd.Dimension = D3D12_RESOURCE_DIMENSION_BUFFER;
  rd.Width = size;
  rd.Height = rd.DepthOrArraySize = rd.MipLevels = 1;
  rd.Format = DXGI_FORMAT_UNKNOWN;
  rd.SampleDesc.Count = 1;
  rd.Layout = D3D12_TEXTURE_LAYOUT_ROW_MAJOR;
  ID3D12Resource *r = NULL;
  if (FAILED(ID3D12Device_CreateCommittedResource(d->dev, &hp, D3D12_HEAP_FLAG_NONE, &rd, state, NULL,
                                                  &k_IID_ID3D12Resource, (void **)&r)))
  {
    return NULL;
  }
  return r;
}

/* `size` bytes of the current frame context's upload ring, `align`ed. 0 when out of memory. */
static int upload_alloc(D12 *d, size_t size, size_t align, Upload *r)
{
  Frame *f = &d->frame[d->cur];
  for (; f->at_chunk < f->n_chunks; f->at_chunk++) {
    Chunk *c = &f->chunks[f->at_chunk];
    const size_t at = (c->used + align - 1) & ~(align - 1);
    if (at + size <= c->size) {
      c->used = at + size;
      r->buf = c->buf;
      r->offset = at;
      r->cpu = c->cpu + at;
      r->gpu = c->gpu + at;
      return 1;
    }
  }
  Chunk *nc = (Chunk *)realloc(f->chunks, sizeof(Chunk) * (size_t)(f->n_chunks + 1));
  if (!nc) {
    return 0;
  }
  f->chunks = nc;
  Chunk *c = &f->chunks[f->n_chunks];
  memset(c, 0, sizeof(*c));
  c->size = size > UPLOAD_CHUNK ? size : UPLOAD_CHUNK;
  c->buf = buffer(d, D3D12_HEAP_TYPE_UPLOAD, c->size, D3D12_RESOURCE_STATE_GENERIC_READ);
  const D3D12_RANGE none = {0, 0};
  if (!c->buf || FAILED(ID3D12Resource_Map(c->buf, 0, &none, (void **)&c->cpu))) {
    REL(c->buf);
    return 0;
  }
  c->gpu = ID3D12Resource_GetGPUVirtualAddress(c->buf);
  f->at_chunk = f->n_chunks++;
  return upload_alloc(d, size, align, r);
}

static int heap_init(D12 *d, CpuHeap *h, D3D12_DESCRIPTOR_HEAP_TYPE type)
{
  h->type = type;
  h->inc = ID3D12Device_GetDescriptorHandleIncrementSize(d->dev, type);
  return 1;
}

/* A free slot of `h` (-1 when none can be made). */
static int slot_alloc(D12 *d, CpuHeap *h)
{
  if (h->n_free > 0) {
    return h->free_slots[--h->n_free];
  }
  if (h->next == h->n_block * HEAP_BLOCK) {
    D3D12_DESCRIPTOR_HEAP_DESC hd = {h->type, HEAP_BLOCK, D3D12_DESCRIPTOR_HEAP_FLAG_NONE, 0};
    int *nf = (int *)realloc(h->free_slots, sizeof(int) * (size_t)(h->n_block + 1) * HEAP_BLOCK);
    if (h->n_block == HEAP_BLOCKS || !nf) {
      return -1;
    }
    h->free_slots = nf;
    if (FAILED(ID3D12Device_CreateDescriptorHeap(d->dev, &hd, &k_IID_ID3D12DescriptorHeap,
                                                 (void **)&h->block[h->n_block])))
    {
      return -1;
    }
    h->start[h->n_block] = cpu_start(h->block[h->n_block]);
    h->n_block++;
  }
  return h->next++;
}

static void slot_free(CpuHeap *h, int slot)
{
  if (slot >= 0) {
    h->free_slots[h->n_free++] = slot;
  }
}

static D3D12_CPU_DESCRIPTOR_HANDLE slot_cpu(const CpuHeap *h, int slot)
{
  D3D12_CPU_DESCRIPTOR_HANDLE c = h->start[slot / HEAP_BLOCK];
  c.ptr += (SIZE_T)(slot % HEAP_BLOCK) * h->inc;
  return c;
}

static void heap_exit(CpuHeap *h)
{
  for (int i = 0; i < h->n_block; i++) {
    REL(h->block[i]);
  }
  free(h->free_slots);
}

/* The current frame context's shader-visible heap holds `need` more descriptors (a context with
 * draws recorded is sent off when it does not: the next one starts empty, and grows if need be). */
static void views_reserve(D12 *d, int need)
{
  Frame *f = &d->frame[d->cur];
  if (f->cap_views - f->n_views >= need) {
    return;
  }
  if (f->n_views > 0) {
    ctx_submit(d);
    ctx_begin(d);
    f = &d->frame[d->cur];
  }
  if (f->cap_views < need) { /* idle (ctx_begin waited for it) and nothing bound from it yet */
    const int cap = need > 2 * f->cap_views ? need : 2 * f->cap_views;
    D3D12_DESCRIPTOR_HEAP_DESC hd = {D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, (UINT)cap,
                                     D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, 0};
    ID3D12DescriptorHeap *nh = NULL;
    if (FAILED(ID3D12Device_CreateDescriptorHeap(d->dev, &hd, &k_IID_ID3D12DescriptorHeap,
                                                 (void **)&nh)))
    {
      return;
    }
    defer_release(d, (IUnknown *)f->views);
    f->views = nh;
    f->views_cpu = cpu_start(nh);
    f->views_gpu = gpu_start(nh);
    f->cap_views = cap;
    ID3D12DescriptorHeap *heaps[2] = {f->views, d->samplers};
    ID3D12GraphicsCommandList_SetDescriptorHeaps(d->list, 2, heaps);
  }
}

/* A descriptor table of the textures' views (NULL: a null view), in the current context. */
static D3D12_GPU_DESCRIPTOR_HANDLE table(D12 *d, BuiGpuTex *const *tex, int n)
{
  Frame *f = &d->frame[d->cur];
  D3D12_GPU_DESCRIPTOR_HANDLE gpu = f->views_gpu;
  if (f->n_views + n > f->cap_views) { /* views_reserve said otherwise: draw with what is there */
    return gpu;
  }
  gpu.ptr += (UINT64)f->n_views * d->view_inc;
  for (int i = 0; i < n; i++) {
    D3D12_CPU_DESCRIPTOR_HANDLE dst = f->views_cpu;
    dst.ptr += (SIZE_T)(f->n_views + i) * d->view_inc;
    ID3D12Device_CopyDescriptorsSimple(d->dev, 1, dst,
                                       slot_cpu(&d->srv_heap, tex[i] ? tex[i]->srv : d->null_srv),
                                       D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV);
  }
  f->n_views += n;
  return gpu;
}

static void barrier(D12 *d, ID3D12Resource *res, D3D12_RESOURCE_STATES from,
                    D3D12_RESOURCE_STATES to)
{
  D3D12_RESOURCE_BARRIER b;
  memset(&b, 0, sizeof(b));
  b.Type = D3D12_RESOURCE_BARRIER_TYPE_TRANSITION;
  b.Transition.pResource = res;
  b.Transition.Subresource = D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES;
  b.Transition.StateBefore = from;
  b.Transition.StateAfter = to;
  ID3D12GraphicsCommandList_ResourceBarrier(d->list, 1, &b);
}

/* `t` in state `s` for what is recorded next. */
static void use(D12 *d, BuiGpuTex *t, D3D12_RESOURCE_STATES s)
{
  if (t->state != s) {
    barrier(d, t->res, t->state, s);
    t->state = s;
  }
}

/* Textures uploaded to since the last draw: sampled again, in one barrier. */
static void pend_flush(D12 *d)
{
  D3D12_RESOURCE_BARRIER b[PEND_MAX];
  int n = 0;
  for (int i = 0; i < d->n_pend; i++) {
    BuiGpuTex *t = d->pend[i];
    t->pending = 0;
    if (t->state != D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE) {
      memset(&b[n], 0, sizeof(b[n]));
      b[n].Type = D3D12_RESOURCE_BARRIER_TYPE_TRANSITION;
      b[n].Transition.pResource = t->res;
      b[n].Transition.Subresource = D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES;
      b[n].Transition.StateBefore = t->state;
      b[n].Transition.StateAfter = D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE;
      t->state = D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE;
      n++;
    }
  }
  d->n_pend = 0;
  if (n) {
    ID3D12GraphicsCommandList_ResourceBarrier(d->list, (UINT)n, b);
  }
}

/** \} */

/* -------------------------------------------------------------------- */
/** \name Textures
 * \{ */

static void d12_tex_free(BuiGpu *g, BuiGpuTex *t)
{
  D12 *d = DEV(g);
  if (t->pending) {
    for (int i = 0; i < d->n_pend; i++) {
      if (d->pend[i] == t) {
        d->pend[i] = d->pend[--d->n_pend];
        break;
      }
    }
  }
  /* views are copied into tables when recorded: their slots are free at once */
  slot_free(&d->srv_heap, t->srv);
  slot_free(&d->rtv_heap, t->rtv);
  defer_release(d, (IUnknown *)t->res);
  free(t);
}

/* A texture; a BUI_TEX_TARGET one starts cleared to `clear`, its optimized clear value. */
static BuiGpuTex *tex_make(D12 *d, BuiGpuFormat fmt, int w, int h, int flags, const float clear[4])
{
  BuiGpuTex *t = d->open ? (BuiGpuTex *)calloc(1, sizeof(BuiGpuTex)) : NULL; /* closed: lost */
  if (!t) {
    return NULL;
  }
  t->srv = t->rtv = -1;
  t->fmt = dxgi_format(fmt, &t->texel);
  const int target = (flags & BUI_TEX_TARGET) != 0;
  D3D12_HEAP_PROPERTIES hp = {D3D12_HEAP_TYPE_DEFAULT, D3D12_CPU_PAGE_PROPERTY_UNKNOWN,
                              D3D12_MEMORY_POOL_UNKNOWN, 1, 1};
  D3D12_RESOURCE_DESC rd;
  memset(&rd, 0, sizeof(rd));
  rd.Dimension = D3D12_RESOURCE_DIMENSION_TEXTURE2D;
  rd.Width = (UINT64)w;
  rd.Height = (UINT)h;
  rd.DepthOrArraySize = rd.MipLevels = 1;
  rd.Format = t->fmt;
  rd.SampleDesc.Count = 1;
  rd.Flags = target ? D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET : D3D12_RESOURCE_FLAG_NONE;
  D3D12_CLEAR_VALUE cv;
  cv.Format = t->fmt;
  memcpy(cv.Color, clear, sizeof(cv.Color));
  /* committed resources start zeroed: one made without data is sampled as it is */
  t->state = target ? D3D12_RESOURCE_STATE_RENDER_TARGET : D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE;
  if (FAILED(ID3D12Device_CreateCommittedResource(d->dev, &hp, D3D12_HEAP_FLAG_NONE, &rd, t->state,
                                                  target ? &cv : NULL, &k_IID_ID3D12Resource,
                                                  (void **)&t->res)) ||
      (t->srv = slot_alloc(d, &d->srv_heap)) < 0 ||
      (target && (t->rtv = slot_alloc(d, &d->rtv_heap)) < 0))
  {
    REL(t->res);
    slot_free(&d->srv_heap, t->srv);
    free(t);
    return NULL;
  }
  D3D12_SHADER_RESOURCE_VIEW_DESC sd;
  memset(&sd, 0, sizeof(sd));
  sd.Format = t->fmt;
  sd.ViewDimension = D3D12_SRV_DIMENSION_TEXTURE2D;
  sd.Shader4ComponentMapping = D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING;
  sd.Texture2D.MipLevels = 1;
  ID3D12Device_CreateShaderResourceView(d->dev, t->res, &sd, slot_cpu(&d->srv_heap, t->srv));
  if (target) {
    ID3D12Device_CreateRenderTargetView(d->dev, t->res, NULL, slot_cpu(&d->rtv_heap, t->rtv));
    ID3D12GraphicsCommandList_ClearRenderTargetView(d->list, slot_cpu(&d->rtv_heap, t->rtv), clear, 0,
                                                    NULL);
  }
  return t;
}

static void d12_tex_update(BuiGpu *g, BuiGpuTex *t, int x, int y, int w, int h, const void *data,
                           int pitch)
{
  D12 *d = DEV(g);
  if (w <= 0 || h <= 0 || !d->open) {
    return;
  }
  const UINT row = (UINT)(w * t->texel);
  const UINT rp = (row + D3D12_TEXTURE_DATA_PITCH_ALIGNMENT - 1) & ~(D3D12_TEXTURE_DATA_PITCH_ALIGNMENT - 1);
  if (!t->pending && d->n_pend == PEND_MAX) {
    pend_flush(d);
  }
  Upload u;
  if (!upload_alloc(d, (size_t)rp * h, D3D12_TEXTURE_DATA_PLACEMENT_ALIGNMENT, &u)) {
    return;
  }
  for (int j = 0; j < h; j++) {
    memcpy(u.cpu + (size_t)rp * j, (const uint8_t *)data + (size_t)pitch * j, row);
  }
  use(d, t, D3D12_RESOURCE_STATE_COPY_DEST);
  if (!t->pending) {
    t->pending = 1;
    d->pend[d->n_pend++] = t;
  }
  D3D12_TEXTURE_COPY_LOCATION dst, src;
  memset(&dst, 0, sizeof(dst));
  memset(&src, 0, sizeof(src));
  dst.pResource = t->res;
  dst.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
  dst.SubresourceIndex = 0;
  src.pResource = u.buf;
  src.Type = D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;
  src.PlacedFootprint.Offset = u.offset;
  src.PlacedFootprint.Footprint.Format = t->fmt;
  src.PlacedFootprint.Footprint.Width = (UINT)w;
  src.PlacedFootprint.Footprint.Height = (UINT)h;
  src.PlacedFootprint.Footprint.Depth = 1;
  src.PlacedFootprint.Footprint.RowPitch = rp;
  ID3D12GraphicsCommandList_CopyTextureRegion(d->list, &dst, (UINT)x, (UINT)y, 0, &src, NULL);
}

static BuiGpuTex *d12_tex_create(BuiGpu *g, BuiGpuFormat fmt, int w, int h, const void *data,
                                 int pitch, int flags)
{
  const float zero[4] = {0, 0, 0, 0};
  BuiGpuTex *t = tex_make(DEV(g), fmt, w, h, flags, zero);
  if (t && data) {
    d12_tex_update(g, t, 0, 0, w, h, data, pitch);
  }
  return t;
}

/** \} */

/* -------------------------------------------------------------------- */
/** \name Pipelines and effects
 * \{ */

/* A pipeline of the root signature drawing into `rt`, premultiplied blending or none. */
static ID3D12PipelineState *pso_make(D12 *d, const void *vs, size_t vs_size, const void *ps,
                                     size_t ps_size, const BuiDxInput *in, UINT n_in,
                                     DXGI_FORMAT rt, int blend)
{
  D3D12_INPUT_ELEMENT_DESC el[8];
  for (UINT k = 0; k < n_in; k++) {
    el[k].SemanticName = in[k].semantic;
    el[k].SemanticIndex = in[k].index;
    el[k].Format = in[k].format;
    el[k].InputSlot = 0;
    el[k].AlignedByteOffset = in[k].offset;
    el[k].InputSlotClass = in[k].instanced ? D3D12_INPUT_CLASSIFICATION_PER_INSTANCE_DATA :
                                             D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA;
    el[k].InstanceDataStepRate = in[k].instanced;
  }
  D3D12_GRAPHICS_PIPELINE_STATE_DESC pd;
  memset(&pd, 0, sizeof(pd));
  pd.pRootSignature = d->root;
  pd.VS.pShaderBytecode = vs;
  pd.VS.BytecodeLength = vs_size;
  pd.PS.pShaderBytecode = ps;
  pd.PS.BytecodeLength = ps_size;
  D3D12_RENDER_TARGET_BLEND_DESC *b = &pd.BlendState.RenderTarget[0];
  b->BlendEnable = blend ? TRUE : FALSE;
  b->SrcBlend = b->SrcBlendAlpha = D3D12_BLEND_ONE;
  b->DestBlend = b->DestBlendAlpha = D3D12_BLEND_INV_SRC_ALPHA;
  b->BlendOp = b->BlendOpAlpha = D3D12_BLEND_OP_ADD;
  b->LogicOp = D3D12_LOGIC_OP_NOOP;
  b->RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL;
  pd.SampleMask = 0xFFFFFFFFu;
  pd.RasterizerState.FillMode = D3D12_FILL_MODE_SOLID;
  pd.RasterizerState.CullMode = D3D12_CULL_MODE_NONE;
  pd.RasterizerState.DepthClipEnable = TRUE;
  pd.DepthStencilState.DepthFunc = D3D12_COMPARISON_FUNC_ALWAYS;
  const D3D12_DEPTH_STENCILOP_DESC keep = {D3D12_STENCIL_OP_KEEP, D3D12_STENCIL_OP_KEEP,
                                           D3D12_STENCIL_OP_KEEP, D3D12_COMPARISON_FUNC_ALWAYS};
  pd.DepthStencilState.FrontFace = pd.DepthStencilState.BackFace = keep;
  pd.InputLayout.pInputElementDescs = n_in ? el : NULL;
  pd.InputLayout.NumElements = n_in;
  pd.PrimitiveTopologyType = D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE;
  pd.NumRenderTargets = 1;
  pd.RTVFormats[0] = rt;
  pd.SampleDesc.Count = 1;
  ID3D12PipelineState *pso = NULL;
  if (FAILED(ID3D12Device_CreateGraphicsPipelineState(d->dev, &pd, &k_IID_ID3D12PipelineState,
                                                       (void **)&pso)))
  {
    return NULL;
  }
  return pso;
}

static void d12_fx_free(BuiGpu *g, BuiGpuProg *p)
{
  D12 *d = DEV(g);
  for (int i = 0; i < FX_PSOS; i++) {
    defer_release(d, (IUnknown *)p->pso[i]);
  }
  REL(p->vs);
  REL(p->ps);
  free(p);
}

static BuiGpuProg *d12_fx_build(BuiGpu *g, const BuiEffectView *v, char *log, size_t log_n)
{
  D12 *d = DEV(g);
  BuiGpuProg *p = (BuiGpuProg *)calloc(1, sizeof(BuiGpuProg));
  if (!p) {
    snprintf(log, log_n, "out of memory");
    return NULL;
  }
  if (!bui_dx_fx_compile(v, "5_0", &p->vs, &p->ps, log, log_n)) {
    free(p);
    return NULL;
  }
  /* the frame's pipeline now, so a shader the device rejects fails here, with its log */
  p->pso[0] = pso_make(d, ID3D10Blob_GetBufferPointer(p->vs), ID3D10Blob_GetBufferSize(p->vs),
                       ID3D10Blob_GetBufferPointer(p->ps), ID3D10Blob_GetBufferSize(p->ps), NULL, 0,
                       DXGI_FORMAT_B8G8R8A8_UNORM, 1);
  if (!p->pso[0]) {
    snprintf(log, log_n, "the device rejected the compiled shader");
    d12_fx_free(g, p);
    return NULL;
  }
  return p;
}

/* Effect `p`'s pipeline for pipeline slot `i` (FX_PSOS), made on first use. */
static ID3D12PipelineState *fx_pso(D12 *d, BuiGpuProg *p, int i)
{
  static const BuiGpuFormat fmt[FX_PSOS] = {BUI_FMT_BGRA8, BUI_FMT_RGBA8, BUI_FMT_RGBA16F,
                                            BUI_FMT_RGBA32F};
  if (!p->pso[i]) {
    int texel;
    p->pso[i] = pso_make(d, ID3D10Blob_GetBufferPointer(p->vs), ID3D10Blob_GetBufferSize(p->vs),
                         ID3D10Blob_GetBufferPointer(p->ps), ID3D10Blob_GetBufferSize(p->ps), NULL,
                         0, dxgi_format(fmt[i], &texel), i == 0);
  }
  return p->pso[i];
}

/** \} */

/* -------------------------------------------------------------------- */
/** \name Device (worker thread)
 * \{ */

typedef HRESULT(WINAPI *CreateDeviceFn)(IUnknown *, D3D_FEATURE_LEVEL, REFIID, void **);
typedef HRESULT(WINAPI *SerializeFn)(const D3D12_ROOT_SIGNATURE_DESC *, D3D_ROOT_SIGNATURE_VERSION,
                                     ID3DBlob **, ID3DBlob **);
typedef HRESULT(WINAPI *GetDebugFn)(REFIID, void **);
typedef HRESULT(WINAPI *FactoryFn)(REFIID, void **);

static int root_init(D12 *d, SerializeFn serialize)
{
  const D3D12_DESCRIPTOR_RANGE tex = {D3D12_DESCRIPTOR_RANGE_TYPE_SRV, TEX_TABLE, 0, 0, 0};
  const D3D12_DESCRIPTOR_RANGE fx = {D3D12_DESCRIPTOR_RANGE_TYPE_SRV, FX_TABLE, 8, 0, 0};
  const D3D12_DESCRIPTOR_RANGE smp = {D3D12_DESCRIPTOR_RANGE_TYPE_SAMPLER, 1, 0, 0, 0};
  D3D12_ROOT_PARAMETER p[5];
  memset(p, 0, sizeof(p));
  p[ROOT_FRAME].ParameterType = D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS;
  p[ROOT_FRAME].Constants.ShaderRegister = 0;
  p[ROOT_FRAME].Constants.Num32BitValues = 4;
  p[ROOT_FRAME].ShaderVisibility = D3D12_SHADER_VISIBILITY_ALL;
  p[ROOT_FX_CB].ParameterType = D3D12_ROOT_PARAMETER_TYPE_CBV;
  p[ROOT_FX_CB].Descriptor.ShaderRegister = 1;
  p[ROOT_FX_CB].ShaderVisibility = D3D12_SHADER_VISIBILITY_ALL;
  const D3D12_DESCRIPTOR_RANGE *ranges[3] = {&tex, &fx, &smp};
  for (int i = 0; i < 3; i++) {
    p[ROOT_TEX + i].ParameterType = D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE;
    p[ROOT_TEX + i].DescriptorTable.NumDescriptorRanges = 1;
    p[ROOT_TEX + i].DescriptorTable.pDescriptorRanges = ranges[i];
    p[ROOT_TEX + i].ShaderVisibility = D3D12_SHADER_VISIBILITY_PIXEL;
  }
  D3D12_ROOT_SIGNATURE_DESC rs = {5, p, 0, NULL,
                                  D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT};
  ID3DBlob *blob = NULL, *err = NULL;
  HRESULT hr = serialize(&rs, D3D_ROOT_SIGNATURE_VERSION_1, &blob, &err);
  if (SUCCEEDED(hr)) {
    hr = ID3D12Device_CreateRootSignature(d->dev, 0, ID3D10Blob_GetBufferPointer(blob),
                                          ID3D10Blob_GetBufferSize(blob), &k_IID_ID3D12RootSignature,
                                          (void **)&d->root);
  }
  REL(blob);
  REL(err);
  return SUCCEEDED(hr) ? 1 : bui_gpu_fail("root signature", hr);
}

static int samplers_init(D12 *d)
{
  D3D12_DESCRIPTOR_HEAP_DESC hd = {D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER, 2,
                                   D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, 0};
  if (FAILED(ID3D12Device_CreateDescriptorHeap(d->dev, &hd, &k_IID_ID3D12DescriptorHeap,
                                               (void **)&d->samplers)))
  {
    return bui_gpu_fail("sampler heap", 0);
  }
  const UINT inc = ID3D12Device_GetDescriptorHandleIncrementSize(d->dev,
                                                                 D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER);
  D3D12_CPU_DESCRIPTOR_HANDLE c = cpu_start(d->samplers);
  D3D12_GPU_DESCRIPTOR_HANDLE gh = gpu_start(d->samplers);
  D3D12_SAMPLER_DESC sd;
  memset(&sd, 0, sizeof(sd));
  sd.AddressU = sd.AddressV = sd.AddressW = D3D12_TEXTURE_ADDRESS_MODE_CLAMP;
  sd.MaxLOD = D3D12_FLOAT32_MAX;
  sd.ComparisonFunc = D3D12_COMPARISON_FUNC_ALWAYS;
  for (int i = 0; i < 2; i++) { /* linear, point (as gpu_d3d11.c) */
    sd.Filter = i ? D3D12_FILTER_MIN_MAG_MIP_POINT : D3D12_FILTER_MIN_MAG_MIP_LINEAR;
    ID3D12Device_CreateSampler(d->dev, &sd, c);
    d->sampler_gpu[i] = gh;
    c.ptr += inc;
    gh.ptr += inc;
  }
  return 1;
}

/* The adapter: BUI_GPU_WARP's software one, BUI_GPU_ADAPTER's, else the first (the display's,
 * which D3D12CreateDevice takes for NULL too). */
static IDXGIAdapter *adapter(D12 *d)
{
  IDXGIAdapter *ad = NULL;
  const char *warp = getenv("BUI_GPU_WARP");
  if (warp && warp[0] == '1') {
    return SUCCEEDED(IDXGIFactory4_EnumWarpAdapter(d->factory, &bui_IID_IDXGIAdapter, (void **)&ad)) ?
               ad :
               NULL;
  }
  ad = bui_dx_preferred_adapter();
  if (!ad && FAILED(IDXGIFactory4_EnumAdapters(d->factory, 0, &ad))) {
    ad = NULL;
  }
  return ad;
}

static int d12_init(BuiGpu *g)
{
  D12 *d = (D12 *)calloc(1, sizeof(D12));
  if (!d) {
    return 0;
  }
  g->dev = d;
  d->null_srv = -1;
  HMODULE dll = LoadLibraryW(L"d3d12.dll");
  HMODULE dxgi = LoadLibraryW(L"dxgi.dll");
  if (!dll || !dxgi) {
    return bui_gpu_fail("loading d3d12.dll", 0);
  }
  CreateDeviceFn create = (CreateDeviceFn)(void *)GetProcAddress(dll, "D3D12CreateDevice");
  SerializeFn serialize = (SerializeFn)(void *)GetProcAddress(dll, "D3D12SerializeRootSignature");
  GetDebugFn get_debug = (GetDebugFn)(void *)GetProcAddress(dll, "D3D12GetDebugInterface");
  FactoryFn factory = (FactoryFn)(void *)GetProcAddress(dxgi, "CreateDXGIFactory1");
  if (!create || !serialize || !factory ||
      FAILED(factory(&bui_IID_IDXGIFactory4, (void **)&d->factory)))
  {
    return bui_gpu_fail("Direct3D 12 runtime", 0);
  }
  if (bui_gpu_debug() >= 2 && get_debug) {
    ID3D12Debug *dbg = NULL;
    if (SUCCEEDED(get_debug(&k_IID_ID3D12Debug, (void **)&dbg))) {
      ID3D12Debug_EnableDebugLayer(dbg);
      REL(dbg);
    }
    else {
      bui_gpu_fail("the Direct3D 12 debug layer (Graphics Tools)", 0);
    }
  }
  IDXGIAdapter *ad = adapter(d);
  HRESULT hr = create((IUnknown *)ad, D3D_FEATURE_LEVEL_11_0, &k_IID_ID3D12Device, (void **)&d->dev);
  if (SUCCEEDED(hr)) {
    bui_dx_report("Direct3D 12", ad);
  }
  REL(ad);
  if (FAILED(hr)) {
    return bui_gpu_fail("D3D12CreateDevice", hr);
  }
  if (bui_gpu_debug() >= 2 &&
      SUCCEEDED(ID3D12Device_QueryInterface(d->dev, &k_IID_ID3D12InfoQueue, (void **)&d->info)))
  {
    D3D12_MESSAGE_SEVERITY deny[] = {D3D12_MESSAGE_SEVERITY_INFO, D3D12_MESSAGE_SEVERITY_MESSAGE};
    /* a performance hint, not a fault: render targets are cleared to any colour
     * (BUI_raster_target_clear), their optimized clear value is the one they start with */
    D3D12_MESSAGE_ID ids[] = {D3D12_MESSAGE_ID_CLEARRENDERTARGETVIEW_MISMATCHINGCLEARVALUE};
    D3D12_INFO_QUEUE_FILTER filter;
    memset(&filter, 0, sizeof(filter));
    filter.DenyList.NumSeverities = 2;
    filter.DenyList.pSeverityList = deny;
    filter.DenyList.NumIDs = 1;
    filter.DenyList.pIDList = ids;
    ID3D12InfoQueue_PushStorageFilter(d->info, &filter);
  }
  g->max_tex = D3D12_REQ_TEXTURE2D_U_OR_V_DIMENSION;
  D3D12_COMMAND_QUEUE_DESC qd = {D3D12_COMMAND_LIST_TYPE_DIRECT, 0, D3D12_COMMAND_QUEUE_FLAG_NONE, 0};
  if (FAILED(ID3D12Device_CreateCommandQueue(d->dev, &qd, &k_IID_ID3D12CommandQueue,
                                             (void **)&d->queue)) ||
      FAILED(ID3D12Device_CreateFence(d->dev, 0, D3D12_FENCE_FLAG_NONE, &k_IID_ID3D12Fence,
                                      (void **)&d->fence)) ||
      !(d->event = CreateEventW(NULL, FALSE, FALSE, NULL)))
  {
    return bui_gpu_fail("command queue", 0);
  }
  d->view_inc = ID3D12Device_GetDescriptorHandleIncrementSize(d->dev,
                                                              D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV);
  for (int i = 0; i < FRAMES; i++) {
    Frame *f = &d->frame[i];
    D3D12_DESCRIPTOR_HEAP_DESC hd = {D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, VIEWS,
                                     D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, 0};
    if (FAILED(ID3D12Device_CreateCommandAllocator(d->dev, D3D12_COMMAND_LIST_TYPE_DIRECT,
                                                   &k_IID_ID3D12CommandAllocator,
                                                   (void **)&f->alloc)) ||
        FAILED(ID3D12Device_CreateDescriptorHeap(d->dev, &hd, &k_IID_ID3D12DescriptorHeap,
                                                 (void **)&f->views)))
    {
      return bui_gpu_fail("frame contexts", 0);
    }
    f->views_cpu = cpu_start(f->views);
    f->views_gpu = gpu_start(f->views);
    f->cap_views = VIEWS;
  }
  if (FAILED(ID3D12Device_CreateCommandList(d->dev, 0, D3D12_COMMAND_LIST_TYPE_DIRECT,
                                            d->frame[0].alloc, NULL,
                                            &k_IID_ID3D12GraphicsCommandList, (void **)&d->list)) ||
      FAILED(ID3D12GraphicsCommandList_Close(d->list)))
  {
    return bui_gpu_fail("command list", 0);
  }
  heap_init(d, &d->srv_heap, D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV);
  heap_init(d, &d->rtv_heap, D3D12_DESCRIPTOR_HEAP_TYPE_RTV);
  if ((d->null_srv = slot_alloc(d, &d->srv_heap)) < 0) {
    return bui_gpu_fail("descriptor heap", 0);
  }
  D3D12_SHADER_RESOURCE_VIEW_DESC nd;
  memset(&nd, 0, sizeof(nd));
  nd.Format = DXGI_FORMAT_R8G8B8A8_UNORM;
  nd.ViewDimension = D3D12_SRV_DIMENSION_TEXTURE2D;
  nd.Shader4ComponentMapping = D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING;
  nd.Texture2D.MipLevels = 1;
  ID3D12Device_CreateShaderResourceView(d->dev, NULL, &nd, slot_cpu(&d->srv_heap, d->null_srv));
  if (!samplers_init(d) || !root_init(d, serialize)) {
    return 0;
  }
  for (int i = 0; i < BUI_DX_PROGRAMS; i++) {
    const BuiDxProgram *p = &bui_dx_programs[i];
    d->pso[i] = pso_make(d, p->vs, p->vs_size, p->ps, p->ps_size, p->inputs, p->n_inputs,
                         DXGI_FORMAT_B8G8R8A8_UNORM, 1);
    if (!d->pso[i]) {
      return bui_gpu_fail("pipeline state", 0);
    }
  }
  ctx_begin(d);
  return !d->lost;
}

static void d12_exit(BuiGpu *g)
{
  D12 *d = DEV(g);
  if (d->queue && d->fence) { /* nothing of ours may be in flight */
    ID3D12CommandQueue_Signal(d->queue, d->fence, ++d->fence_value);
    wait_fence(d, d->fence_value);
  }
  if (d->target) {
    d12_tex_free(g, d->target);
  }
  for (int i = 0; i < 2; i++) {
    REL(d->back[i]);
  }
  REL(d->swap);
  REL(d->dcomp_visual);
  REL(d->dcomp_target);
  REL(d->dcomp);
  for (int i = 0; i < FRAMES; i++) {
    Frame *f = &d->frame[i];
    frame_release(f);
    free(f->dead);
    for (int k = 0; k < f->n_chunks; k++) {
      REL(f->chunks[k].buf);
    }
    free(f->chunks);
    REL(f->views);
    REL(f->alloc);
  }
  for (int i = 0; i < BUI_DX_PROGRAMS; i++) {
    REL(d->pso[i]);
  }
  REL(d->root);
  REL(d->samplers);
  heap_exit(&d->srv_heap);
  heap_exit(&d->rtv_heap);
  REL(d->list);
  REL(d->fence);
  if (d->event) {
    CloseHandle(d->event);
  }
  REL(d->queue);
  REL(d->factory);
  if (d->info) { /* what is still alive now is a leak: the device is all that should be left */
    debug_drain(d, 0);
    ID3D12DebugDevice *dd = NULL;
    if (SUCCEEDED(ID3D12Device_QueryInterface(d->dev, &k_IID_ID3D12DebugDevice, (void **)&dd))) {
      ID3D12DebugDevice_ReportLiveDeviceObjects(dd, D3D12_RLDO_DETAIL | D3D12_RLDO_IGNORE_INTERNAL);
      REL(dd);
    }
    debug_drain(d, 1);
    fprintf(stderr, "bui gpu: d3d12 debug layer: %d errors (leaks included), %d warnings\n",
            d->n_errors,
            d->n_warnings);
  }
  REL(d->info);
  REL(d->dev);
  free(d);
}

/** \} */

/* -------------------------------------------------------------------- */
/** \name Drawing + present
 * \{ */

static D3D12_CPU_DESCRIPTOR_HANDLE rtv_of(const D12 *d, const BuiGpuTex *t)
{
  return slot_cpu(&d->rtv_heap, t->rtv);
}

static int d12_target(BuiGpu *g, int tw, int th)
{
  D12 *d = DEV(g);
  const float clear[4] = {0, 0, 0, g->alpha ? 0.0f : 1.0f};
  if (!d->target || d->tw != tw || d->th != th) {
    if (d->target) {
      d12_tex_free(g, d->target);
    }
    d->target = tex_make(d, BUI_FMT_BGRA8, tw, th, BUI_TEX_TARGET, clear);
    if (!d->target) {
      return 0;
    }
    d->tw = tw;
    d->th = th;
    return 1; /* made cleared */
  }
  use(d, d->target, D3D12_RESOURCE_STATE_RENDER_TARGET);
  ID3D12GraphicsCommandList_ClearRenderTargetView(d->list, rtv_of(d, d->target), clear, 0, NULL);
  return 1;
}

static void viewport(D12 *d, int w, int h)
{
  const D3D12_VIEWPORT vp = {0, 0, (float)w, (float)h, 0, 1};
  ID3D12GraphicsCommandList_RSSetViewports(d->list, 1, &vp);
}

static void scissor(D12 *d, int x0, int y0, int x1, int y1)
{
  const D3D12_RECT r = {x0, y0, x1, y1};
  ID3D12GraphicsCommandList_RSSetScissorRects(d->list, 1, &r);
}

/* One recorded effect draw: its pipeline, floats and inputs, scissored to its clip. */
static void fx_draw(BuiGpu *g, const BuiGpuFxCall *fc)
{
  D12 *d = DEV(g);
  ID3D12GraphicsCommandList *c = d->list;
  /* a pass into a render target: its view and size stand in for the frame's until it is drawn */
  const BuiGpuDevTex *dst = fc->dst ? bui_gpu_dtex(g, fc->dst) : NULL;
  if (fc->dst && (!dst || dst->tex->rtv < 0)) {
    return;
  }
  if (!fc->effect) { /* a clear of the target */
    use(d, dst->tex, D3D12_RESOURCE_STATE_RENDER_TARGET);
    ID3D12GraphicsCommandList_ClearRenderTargetView(c, rtv_of(d, dst->tex), g->fxf + fc->floats, 0,
                                                    NULL);
    return;
  }
  const BuiGpuDevFx *f = bui_gpu_dfx(g, fc->effect);
  ID3D12PipelineState *pso = f ? fx_pso(d, f->prog, dst ? dst->target : 0) : NULL;
  Upload u;
  BuiGpuFxConsts consts;
  const size_t n = f ? bui_gpu_fx_consts(g, fc, f->n_floats, dst ? dst->w : d->tw,
                                         dst ? dst->h : d->th, &consts) :
                       0;
  if (!pso || !upload_alloc(d, n, D3D12_CONSTANT_BUFFER_DATA_PLACEMENT_ALIGNMENT, &u)) {
    return;
  }
  memcpy(u.cpu, consts.data, n);
  BuiGpuTex *in[FX_TABLE] = {0};
  for (int k = 0; k < f->n_inputs; k++) {
    in[k] = bui_gpu_dtex_tex(g, fc->inputs[k]);
    use(d, in[k], D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE);
  }
  ID3D12GraphicsCommandList_SetGraphicsRootDescriptorTable(c, ROOT_FX_TEX, table(d, in, FX_TABLE));
  ID3D12GraphicsCommandList_SetGraphicsRootConstantBufferView(c, ROOT_FX_CB, u.gpu);
  ID3D12GraphicsCommandList_SetGraphicsRootDescriptorTable(c, ROOT_SAMPLER, d->sampler_gpu[0]);
  if (dst) {
    use(d, dst->tex, D3D12_RESOURCE_STATE_RENDER_TARGET);
    const D3D12_CPU_DESCRIPTOR_HANDLE rtv = rtv_of(d, dst->tex);
    ID3D12GraphicsCommandList_OMSetRenderTargets(c, 1, &rtv, FALSE, NULL);
    viewport(d, dst->w, dst->h);
  }
  ID3D12GraphicsCommandList_SetPipelineState(c, pso);
  scissor(d, fc->clip[0], fc->clip[1], fc->clip[2], fc->clip[3]);
  ID3D12GraphicsCommandList_DrawInstanced(c, 4, 1, 0, 0);
  if (dst) { /* back to the frame */
    const D3D12_CPU_DESCRIPTOR_HANDLE rtv = rtv_of(d, d->target);
    ID3D12GraphicsCommandList_OMSetRenderTargets(c, 1, &rtv, FALSE, NULL);
    viewport(d, d->tw, d->th);
  }
}

/* The vertex data of batch kind `k` in the upload ring. */
static D3D12_VERTEX_BUFFER_VIEW vertices(D12 *d, const BuiGpu *g, int k)
{
  D3D12_VERTEX_BUFFER_VIEW v = {0, 0, bui_dx_programs[k].stride};
  int n;
  const void *data = bui_gpu_batch_data(g, (BuiGpuBatchKind)k, &n);
  Upload u;
  if (n && upload_alloc(d, (size_t)v.StrideInBytes * n, 16, &u)) {
    memcpy(u.cpu, data, (size_t)v.StrideInBytes * n);
    v.BufferLocation = u.gpu;
    v.SizeInBytes = v.StrideInBytes * (UINT)n;
  }
  return v;
}

static void d12_draw(BuiGpu *g)
{
  D12 *d = DEV(g);
  if (d->lost || !d->open) {
    return;
  }
  views_reserve(d, (g->n_b + 1) * (TEX_TABLE + FX_TABLE));
  ID3D12GraphicsCommandList *c = d->list;
  D3D12_VERTEX_BUFFER_VIEW vb[BUI_DX_PROGRAMS];
  for (int k = 0; k < BUI_DX_PROGRAMS; k++) {
    vb[k] = vertices(d, g, k);
  }
  D3D12_INDEX_BUFFER_VIEW ib = {0, 0, DXGI_FORMAT_R32_UINT};
  Upload u;
  if (g->n_mi && upload_alloc(d, sizeof(uint32_t) * (size_t)g->n_mi, 16, &u)) {
    memcpy(u.cpu, g->mi, sizeof(uint32_t) * (size_t)g->n_mi);
    ib.BufferLocation = u.gpu;
    ib.SizeInBytes = (UINT)(sizeof(uint32_t) * (size_t)g->n_mi);
  }
  pend_flush(d);
  use(d, d->target, D3D12_RESOURCE_STATE_RENDER_TARGET);
  const D3D12_CPU_DESCRIPTOR_HANDLE rtv = rtv_of(d, d->target);
  ID3D12GraphicsCommandList_OMSetRenderTargets(c, 1, &rtv, FALSE, NULL);
  viewport(d, d->tw, d->th);
  const float cbd[4] = {2.0f / (float)d->tw, 2.0f / (float)d->th, 0, 0};
  ID3D12GraphicsCommandList_SetGraphicsRoot32BitConstants(c, ROOT_FRAME, 4, cbd, 0);

  /* t0..t5: mask atlas, image atlas (or a big image), Slug curves, Slug bands, mesh texture, NV12
   * chroma */
  BuiGpuTex *tex[TEX_TABLE] = {g->mask.tex, g->rgba.tex, g->curve_tex, g->band_tex, NULL, NULL};
  int tex_dirty = 1;
  int cur = -1;
  for (int i = 0; i < g->n_b; i++) {
    const BuiGpuBatch *b = &g->b[i];
    if (b->count == 0) {
      continue;
    }
    if ((int)b->kind != cur) {
      cur = (int)b->kind;
      /* quads: 4 vertices from SV_VertexID per instance, as effects (which have no vertex data) */
      ID3D12GraphicsCommandList_IASetPrimitiveTopology(
          c, b->kind == BUI_BATCH_QUADS || b->kind == BUI_BATCH_FX ? D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP :
                                                                     D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
      if (b->kind != BUI_BATCH_FX) {
        ID3D12GraphicsCommandList_SetPipelineState(c, d->pso[b->kind]);
        ID3D12GraphicsCommandList_IASetVertexBuffers(c, 0, 1, &vb[b->kind]);
      }
      if (b->kind == BUI_BATCH_MESH) {
        ID3D12GraphicsCommandList_IASetIndexBuffer(c, &ib);
      }
      else if (b->kind != BUI_BATCH_FX) { /* scissored: meshes and effects, to their clip */
        scissor(d, 0, 0, d->tw, d->th);
      }
    }
    if (b->kind == BUI_BATCH_MESH) {
      BuiGpuTex *t = bui_gpu_dtex_tex(g, b->tex);
      use(d, t, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE);
      tex_dirty |= tex[4] != t;
      tex[4] = t;
      ID3D12GraphicsCommandList_SetGraphicsRootDescriptorTable(c, ROOT_SAMPLER,
                                                               d->sampler_gpu[b->nearest ? 1 : 0]);
      scissor(d, b->clip[0], b->clip[1], b->clip[2], b->clip[3]);
    }
    else if (b->kind == BUI_BATCH_QUADS) {
      BuiGpuTex *uv;
      BuiGpuTex *img = bui_gpu_batch_image(g, b, &uv);
      tex_dirty |= tex[1] != img || tex[5] != uv;
      tex[1] = img;
      tex[5] = uv;
    }
    if (tex_dirty && (b->kind == BUI_BATCH_MESH || b->kind == BUI_BATCH_QUADS)) {
      tex_dirty = 0;
      ID3D12GraphicsCommandList_SetGraphicsRootDescriptorTable(c, ROOT_TEX, table(d, tex, TEX_TABLE));
    }
    if (b->kind == BUI_BATCH_MESH) {
      ID3D12GraphicsCommandList_DrawIndexedInstanced(c, (UINT)b->count, 1, (UINT)b->first, 0, 0);
    }
    else if (b->kind == BUI_BATCH_QUADS) {
      ID3D12GraphicsCommandList_DrawInstanced(c, 4, (UINT)b->count, 0, (UINT)b->first);
    }
    else if (b->kind == BUI_BATCH_FX) {
      fx_draw(g, &g->fx[b->first]);
      cur = -1; /* its pipeline replaced the renderer's */
    }
    else {
      ID3D12GraphicsCommandList_DrawInstanced(c, (UINT)b->count, 1, (UINT)b->first, 0);
    }
  }
}

static void d12_clear(BuiGpu *g, const RECT *r)
{
  D12 *d = DEV(g);
  const float clear[4] = {0, 0, 0, 0};
  use(d, d->target, D3D12_RESOURCE_STATE_RENDER_TARGET);
  ID3D12GraphicsCommandList_ClearRenderTargetView(d->list, rtv_of(d, d->target), clear, 1, r);
}

static int back_buffers(D12 *d)
{
  for (UINT i = 0; i < 2; i++) {
    if (FAILED(IDXGISwapChain3_GetBuffer(d->swap, i, &k_IID_ID3D12Resource, (void **)&d->back[i]))) {
      return 0;
    }
  }
  return 1;
}

/* Shows composition swap chain `swap` in the window (alpha windows): a DirectComposition visual
 * over the window's own surface. */
static int dcomp_show(D12 *d, HWND hwnd, IDXGISwapChain1 *swap)
{
  typedef HRESULT(WINAPI * CreateFn)(IUnknown *, REFIID, void **);
  HMODULE dll = LoadLibraryW(L"dcomp.dll");
  CreateFn create = dll ? (CreateFn)(void *)GetProcAddress(dll, "DCompositionCreateDevice") : NULL;
  if (!create || FAILED(create(NULL, &k_IID_IDCompositionDevice, (void **)&d->dcomp))) {
    return bui_gpu_fail("DirectComposition", 0);
  }
  const DcDeviceVtbl *dv = (const DcDeviceVtbl *)d->dcomp->vtbl;
  if (FAILED(dv->CreateTargetForHwnd(d->dcomp, hwnd, TRUE, &d->dcomp_target)) ||
      FAILED(dv->CreateVisual(d->dcomp, &d->dcomp_visual)) ||
      FAILED(((const DcVisualVtbl *)d->dcomp_visual->vtbl)->SetContent(d->dcomp_visual,
                                                                        (IUnknown *)swap)) ||
      FAILED(((const DcTargetVtbl *)d->dcomp_target->vtbl)->SetRoot(d->dcomp_target,
                                                                     d->dcomp_visual)) ||
      FAILED(dv->Commit(d->dcomp)))
  {
    return bui_gpu_fail("DirectComposition visual", 0);
  }
  return 1;
}

static int d12_swap(BuiGpu *g, int cw, int ch)
{
  D12 *d = DEV(g);
  if (d->swap) { /* no buffer may be in use or referenced */
    gpu_idle(d);
    REL(d->back[0]);
    REL(d->back[1]);
    return SUCCEEDED(IDXGISwapChain3_ResizeBuffers(d->swap, 0, (UINT)cw, (UINT)ch,
                                                   DXGI_FORMAT_UNKNOWN, 0)) &&
           back_buffers(d);
  }
  DXGI_SWAP_CHAIN_DESC1 sd = bui_dx_swap_desc(cw, ch);
  IDXGISwapChain1 *s1 = NULL;
  HRESULT hr;
  if (g->alpha) {
    /* a window's flip-model swap chain ignores alpha; a composition one keeps it */
    sd.AlphaMode = DXGI_ALPHA_MODE_PREMULTIPLIED;
    sd.Scaling = DXGI_SCALING_STRETCH;
    hr = IDXGIFactory4_CreateSwapChainForComposition(d->factory, (IUnknown *)d->queue, &sd, NULL, &s1);
    if (SUCCEEDED(hr) && !dcomp_show(d, g->hwnd, s1)) {
      hr = E_FAIL;
    }
  }
  else {
    hr = IDXGIFactory4_CreateSwapChainForHwnd(d->factory, (IUnknown *)d->queue, g->hwnd, &sd, NULL,
                                              NULL, &s1);
    if (SUCCEEDED(hr)) {
      IDXGIFactory4_MakeWindowAssociation(d->factory, g->hwnd, DXGI_MWA_NO_ALT_ENTER);
    }
  }
  if (SUCCEEDED(hr)) {
    hr = IDXGISwapChain1_QueryInterface(s1, &k_IID_IDXGISwapChain3, (void **)&d->swap);
  }
  REL(s1);
  if (FAILED(hr) || !back_buffers(d)) {
    REL(d->back[0]);
    REL(d->back[1]);
    REL(d->swap);
    REL(d->dcomp_visual);
    REL(d->dcomp_target);
    REL(d->dcomp);
    return bui_gpu_fail("swap chain", hr);
  }
  return 1;
}

static void copy_box(D12 *d, ID3D12Resource *dst, const D3D12_BOX *b)
{
  D3D12_TEXTURE_COPY_LOCATION to, from;
  memset(&to, 0, sizeof(to));
  memset(&from, 0, sizeof(from));
  to.pResource = dst;
  to.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
  from.pResource = d->target->res;
  from.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
  ID3D12GraphicsCommandList_CopyTextureRegion(d->list, &to, b->left, b->top, 0, &from, b);
}

static int d12_present(BuiGpu *g, int whole, const RECT *copy, int n_copy, const RECT *dirty,
                       int n_dirty)
{
  D12 *d = DEV(g);
  if (d->lost || !d->open) {
    return BUI_PRESENT_LOST;
  }
  ID3D12Resource *back = d->back[IDXGISwapChain3_GetCurrentBackBufferIndex(d->swap) & 1];
  barrier(d, back, D3D12_RESOURCE_STATE_PRESENT, D3D12_RESOURCE_STATE_COPY_DEST);
  use(d, d->target, D3D12_RESOURCE_STATE_COPY_SOURCE);
  if (whole) {
    /* the target can be larger than the window (it grows in steps): copy the frame, not the resource */
    const D3D12_BOX frame = {0, 0, 0, (UINT)g->w, (UINT)g->h, 1};
    copy_box(d, back, &frame);
  }
  for (int i = 0; i < n_copy; i++) {
    const D3D12_BOX b = {(UINT)copy[i].left, (UINT)copy[i].top, 0, (UINT)copy[i].right,
                         (UINT)copy[i].bottom, 1};
    copy_box(d, back, &b);
  }
  barrier(d, back, D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_PRESENT);
  use(d, d->target, D3D12_RESOURCE_STATE_RENDER_TARGET);
  ctx_submit(d);
  DXGI_PRESENT_PARAMETERS pp = {0};
  pp.DirtyRectsCount = (UINT)n_dirty;
  pp.pDirtyRects = n_dirty ? (RECT *)dirty : NULL;
  const int st = bui_dx_present_status(IDXGISwapChain3_Present1(d->swap, 0, 0, &pp));
  ctx_begin(d);
  return d->lost ? BUI_PRESENT_LOST : st;
}

static void d12_submit(BuiGpu *g)
{
  D12 *d = DEV(g);
  if (d->open) {
    ctx_submit(d);
    ctx_begin(d);
  }
}

static int d12_read(BuiGpu *g, uint32_t *dst, int w, int h)
{
  D12 *d = DEV(g);
  if (d->lost || !d->open) {
    return 0;
  }
  const UINT rp = ((UINT)w * 4 + D3D12_TEXTURE_DATA_PITCH_ALIGNMENT - 1) &
                  ~(D3D12_TEXTURE_DATA_PITCH_ALIGNMENT - 1);
  const size_t size = (size_t)rp * h;
  ID3D12Resource *rb = buffer(d, D3D12_HEAP_TYPE_READBACK, size, D3D12_RESOURCE_STATE_COPY_DEST);
  if (!rb) {
    return 0;
  }
  D3D12_TEXTURE_COPY_LOCATION to, from;
  memset(&to, 0, sizeof(to));
  memset(&from, 0, sizeof(from));
  to.pResource = rb;
  to.Type = D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;
  to.PlacedFootprint.Footprint.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
  to.PlacedFootprint.Footprint.Width = (UINT)w;
  to.PlacedFootprint.Footprint.Height = (UINT)h;
  to.PlacedFootprint.Footprint.Depth = 1;
  to.PlacedFootprint.Footprint.RowPitch = rp;
  from.pResource = d->target->res;
  from.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
  const D3D12_BOX frame = {0, 0, 0, (UINT)w, (UINT)h, 1};
  use(d, d->target, D3D12_RESOURCE_STATE_COPY_SOURCE);
  ID3D12GraphicsCommandList_CopyTextureRegion(d->list, &to, 0, 0, 0, &from, &frame);
  use(d, d->target, D3D12_RESOURCE_STATE_RENDER_TARGET);
  gpu_idle(d);
  const D3D12_RANGE all = {0, size}, none = {0, 0};
  uint8_t *p = NULL;
  int ok = 0;
  if (!d->lost && SUCCEEDED(ID3D12Resource_Map(rb, 0, &all, (void **)&p))) {
    for (int y = 0; y < h; y++) {
      memcpy(dst + (size_t)y * w, p + (size_t)y * rp, sizeof(uint32_t) * (size_t)w);
    }
    ID3D12Resource_Unmap(rb, 0, &none);
    ok = 1;
  }
  REL(rb); /* idle: nothing uses it */
  return ok;
}

/** \} */

const BuiGpuOps bui_gpu_d3d12_ops = {
    BUI_GPU_API_D3D12,
    d12_init,
    d12_exit,
    d12_tex_create,
    d12_tex_update,
    d12_tex_free,
    d12_fx_build,
    d12_fx_free,
    d12_target,
    d12_draw,
    d12_clear,
    d12_swap,
    d12_present,
    d12_submit,
    d12_read,
};

#endif /* _WIN32 && BUI_GPU */
