/* BUI GPU renderer: the Direct3D 11 backend of gpu_device.c (the default on Windows).
 *
 * Every primitive is one instance of the uber-shader in gpu/shaders/bui_ui.hlsl, drawn with
 * DrawInstanced into a persistent render target; present copies the damaged rects into a
 * flip-model swap chain (a bitblt one for alpha windows) and presents them as dirty rects.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */

#if defined(_WIN32) && defined(BUI_GPU) /* BUI_GPU: features.json */

#  define COBJMACROS
#  include "gpu_dx.h"

#  include <d3d11.h>
#  include <d3d11_1.h>

#  include <stdio.h>
#  include <stdlib.h>
#  include <string.h>

static const GUID k_IID_IDXGIDevice = {
    0x54ec77fa, 0x1377, 0x44e6, {0x8c, 0x32, 0x88, 0xfd, 0x5f, 0x44, 0xc8, 0x4c}};
static const GUID k_IID_ID3D11Texture2D = {
    0x6f15aaf2, 0xd208, 0x4e89, {0x9a, 0xb4, 0x48, 0x95, 0x35, 0xd3, 0x4f, 0x9c}};
static const GUID k_IID_ID3D11DeviceContext1 = {
    0xbb2c6faa, 0xb5fb, 0x4082, {0x8e, 0x6b, 0x38, 0x8b, 0x8c, 0xfa, 0x90, 0xe1}};

struct BuiGpuTex {
  ID3D11Texture2D *tex;
  ID3D11ShaderResourceView *srv;
  ID3D11RenderTargetView *rtv; /* BUI_TEX_TARGET */
};

struct BuiGpuProg {
  ID3D11VertexShader *vs;
  ID3D11PixelShader *ps;
  ID3D11Buffer *cb; /* b1: BuiGpuFxConsts */
};

typedef struct D11 {
  ID3D11Device *dev;
  ID3D11DeviceContext *ctx;
  ID3D11DeviceContext1 *ctx1; /* alpha frames: ClearView clears the rects of a redrawn region */
  IDXGIFactory2 *factory;
  IDXGISwapChain1 *swap;

  /* by batch kind (bui_dx_programs) */
  ID3D11VertexShader *vs[BUI_DX_PROGRAMS];
  ID3D11PixelShader *ps[BUI_DX_PROGRAMS];
  ID3D11InputLayout *il[BUI_DX_PROGRAMS];
  ID3D11Buffer *cb, *vb[BUI_DX_PROGRAMS], *mib; /* vertex data by batch kind, mesh indices */
  UINT vb_cap[BUI_DX_PROGRAMS], mib_cap;          /* in elements */
  ID3D11BlendState *blend;
  ID3D11RasterizerState *rs, *rs_scissor;
  ID3D11SamplerState *smp_linear, *smp_point;

  ID3D11Texture2D *target;
  ID3D11RenderTargetView *rtv;
  int tw, th;
} D11;

#  define DEV(g) ((D11 *)(g)->dev)

static DXGI_FORMAT dxgi_format(BuiGpuFormat fmt)
{
  static const DXGI_FORMAT map[] = {
      DXGI_FORMAT_R8_UNORM,           DXGI_FORMAT_R8G8_UNORM,         DXGI_FORMAT_B8G8R8A8_UNORM,
      DXGI_FORMAT_R8G8B8A8_UNORM,     DXGI_FORMAT_R16G16B16A16_FLOAT, DXGI_FORMAT_R32G32B32A32_FLOAT,
      DXGI_FORMAT_R32G32_UINT,
  };
  return map[fmt];
}

/* -------------------------------------------------------------------- */
/** \name Device (worker thread)
 * \{ */

typedef HRESULT(WINAPI *CreateDeviceFn)(IDXGIAdapter *, D3D_DRIVER_TYPE, HMODULE, UINT,
                                        const D3D_FEATURE_LEVEL *, UINT, UINT, ID3D11Device **,
                                        D3D_FEATURE_LEVEL *, ID3D11DeviceContext **);

static int d11_init(BuiGpu *g)
{
  D11 *d = (D11 *)calloc(1, sizeof(D11));
  if (!d) {
    return 0;
  }
  g->dev = d;
  HMODULE dll = LoadLibraryW(L"d3d11.dll");
  if (!dll) {
    return bui_gpu_fail("loading d3d11.dll", 0);
  }
  CreateDeviceFn create = (CreateDeviceFn)(void *)GetProcAddress(dll, "D3D11CreateDevice");
  if (!create) {
    return 0;
  }
  const D3D_FEATURE_LEVEL levels[] = {D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0,
                                      D3D_FEATURE_LEVEL_10_1, D3D_FEATURE_LEVEL_10_0};
  const char *warp = getenv("BUI_GPU_WARP");
  const D3D_DRIVER_TYPE type = (warp && warp[0] == '1') ? D3D_DRIVER_TYPE_WARP :
                                                          D3D_DRIVER_TYPE_HARDWARE;
  /* an explicit adapter takes D3D_DRIVER_TYPE_UNKNOWN */
  IDXGIAdapter *pref = type == D3D_DRIVER_TYPE_HARDWARE ? bui_dx_preferred_adapter() : NULL;
  const D3D_DRIVER_TYPE driver = pref ? D3D_DRIVER_TYPE_UNKNOWN : type;
  D3D_FEATURE_LEVEL got;
  HRESULT hr = create(pref, driver, NULL, D3D11_CREATE_DEVICE_BGRA_SUPPORT, levels, 4,
                      D3D11_SDK_VERSION, &d->dev, &got, &d->ctx);
  if (FAILED(hr)) { /* 11_1 unknown to the runtime: retry without it */
    hr = create(pref, driver, NULL, D3D11_CREATE_DEVICE_BGRA_SUPPORT, levels + 1, 3,
                D3D11_SDK_VERSION, &d->dev, &got, &d->ctx);
  }
  REL(pref);
  if (FAILED(hr)) {
    return bui_gpu_fail("D3D11CreateDevice", hr);
  }
  if (g->alpha && FAILED(hr = ID3D11DeviceContext_QueryInterface(d->ctx, &k_IID_ID3D11DeviceContext1,
                                                                  (void **)&d->ctx1)))
  {
    return bui_gpu_fail("ID3D11DeviceContext1", hr); /* Windows 8: ClearView */
  }
  g->max_tex = got >= D3D_FEATURE_LEVEL_11_0 ? D3D11_REQ_TEXTURE2D_U_OR_V_DIMENSION : 8192;
  IDXGIDevice *xd = NULL;
  IDXGIAdapter *ad = NULL;
  if (FAILED(ID3D11Device_QueryInterface(d->dev, &k_IID_IDXGIDevice, (void **)&xd)) ||
      FAILED(IDXGIDevice_GetAdapter(xd, &ad)) ||
      FAILED(IDXGIAdapter_GetParent(ad, &bui_IID_IDXGIFactory2, (void **)&d->factory)))
  {
    REL(ad);
    REL(xd);
    return bui_gpu_fail("DXGI factory", 0);
  }
  bui_dx_report("Direct3D 11", ad);
  REL(ad);
  REL(xd);

  ID3D11Device *dv = d->dev;
  for (int i = 0; i < BUI_DX_PROGRAMS; i++) {
    const BuiDxProgram *p = &bui_dx_programs[i];
    D3D11_INPUT_ELEMENT_DESC el[8];
    for (UINT k = 0; k < p->n_inputs; k++) {
      const BuiDxInput *in = &p->inputs[k];
      const D3D11_INPUT_ELEMENT_DESC e = {in->semantic, in->index, in->format, 0, in->offset,
                                          in->instanced ? D3D11_INPUT_PER_INSTANCE_DATA :
                                                          D3D11_INPUT_PER_VERTEX_DATA,
                                          in->instanced};
      el[k] = e;
    }
    if (FAILED(ID3D11Device_CreateVertexShader(dv, p->vs, p->vs_size, NULL, &d->vs[i])) ||
        FAILED(ID3D11Device_CreatePixelShader(dv, p->ps, p->ps_size, NULL, &d->ps[i])) ||
        FAILED(ID3D11Device_CreateInputLayout(dv, el, p->n_inputs, p->vs, p->vs_size, &d->il[i])))
    {
      return bui_gpu_fail("shaders", 0);
    }
  }
  D3D11_BUFFER_DESC bd = {0};
  bd.ByteWidth = 16;
  bd.Usage = D3D11_USAGE_DYNAMIC;
  bd.BindFlags = D3D11_BIND_CONSTANT_BUFFER;
  bd.CPUAccessFlags = D3D11_CPU_ACCESS_WRITE;
  if (FAILED(ID3D11Device_CreateBuffer(dv, &bd, NULL, &d->cb))) {
    return bui_gpu_fail("constant buffer", 0);
  }
  D3D11_BLEND_DESC bl = {0};
  bl.RenderTarget[0].BlendEnable = TRUE;
  bl.RenderTarget[0].SrcBlend = D3D11_BLEND_ONE;
  bl.RenderTarget[0].DestBlend = D3D11_BLEND_INV_SRC_ALPHA;
  bl.RenderTarget[0].BlendOp = D3D11_BLEND_OP_ADD;
  bl.RenderTarget[0].SrcBlendAlpha = D3D11_BLEND_ONE;
  bl.RenderTarget[0].DestBlendAlpha = D3D11_BLEND_INV_SRC_ALPHA;
  bl.RenderTarget[0].BlendOpAlpha = D3D11_BLEND_OP_ADD;
  bl.RenderTarget[0].RenderTargetWriteMask = D3D11_COLOR_WRITE_ENABLE_ALL;
  if (FAILED(ID3D11Device_CreateBlendState(dv, &bl, &d->blend))) {
    return bui_gpu_fail("blend state", 0);
  }
  D3D11_RASTERIZER_DESC rd = {0};
  rd.FillMode = D3D11_FILL_SOLID;
  rd.CullMode = D3D11_CULL_NONE;
  rd.DepthClipEnable = TRUE;
  if (FAILED(ID3D11Device_CreateRasterizerState(dv, &rd, &d->rs))) {
    return bui_gpu_fail("rasterizer state", 0);
  }
  rd.ScissorEnable = TRUE; /* meshes: the draw command's clip */
  if (FAILED(ID3D11Device_CreateRasterizerState(dv, &rd, &d->rs_scissor))) {
    return bui_gpu_fail("rasterizer state", 0);
  }
  D3D11_SAMPLER_DESC sd = {0};
  sd.Filter = D3D11_FILTER_MIN_MAG_MIP_LINEAR;
  sd.AddressU = sd.AddressV = sd.AddressW = D3D11_TEXTURE_ADDRESS_CLAMP;
  sd.MaxLOD = D3D11_FLOAT32_MAX;
  sd.ComparisonFunc = D3D11_COMPARISON_ALWAYS;
  if (FAILED(ID3D11Device_CreateSamplerState(dv, &sd, &d->smp_linear))) {
    return bui_gpu_fail("sampler", 0);
  }
  sd.Filter = D3D11_FILTER_MIN_MAG_MIP_POINT;
  if (FAILED(ID3D11Device_CreateSamplerState(dv, &sd, &d->smp_point))) {
    return bui_gpu_fail("sampler", 0);
  }
  return 1;
}

static void d11_exit(BuiGpu *g)
{
  D11 *d = DEV(g);
  if (d->ctx) {
    ID3D11DeviceContext_ClearState(d->ctx);
  }
  REL(d->smp_linear);
  REL(d->smp_point);
  REL(d->rs_scissor);
  REL(d->mib);
  for (int i = 0; i < BUI_DX_PROGRAMS; i++) {
    REL(d->vb[i]);
    REL(d->il[i]);
    REL(d->vs[i]);
    REL(d->ps[i]);
  }
  REL(d->rtv);
  REL(d->target);
  REL(d->rs);
  REL(d->blend);
  REL(d->cb);
  REL(d->swap);
  REL(d->factory);
  REL(d->ctx1);
  REL(d->ctx);
  REL(d->dev);
  free(d);
}

/** \} */

/* -------------------------------------------------------------------- */
/** \name Textures and effects
 * \{ */

static void d11_tex_free(BuiGpu *g, BuiGpuTex *t)
{
  (void)g;
  REL(t->rtv);
  REL(t->srv);
  REL(t->tex);
  free(t);
}

static BuiGpuTex *d11_tex_create(BuiGpu *g, BuiGpuFormat fmt, int w, int h, const void *data,
                                 int pitch, int flags)
{
  D11 *d = DEV(g);
  BuiGpuTex *t = (BuiGpuTex *)calloc(1, sizeof(BuiGpuTex));
  if (!t) {
    return NULL;
  }
  D3D11_TEXTURE2D_DESC td = {0};
  td.Width = (UINT)w;
  td.Height = (UINT)h;
  td.MipLevels = td.ArraySize = 1;
  td.Format = dxgi_format(fmt);
  td.SampleDesc.Count = 1;
  td.Usage = (flags & BUI_TEX_IMMUTABLE) && data ? D3D11_USAGE_IMMUTABLE : D3D11_USAGE_DEFAULT;
  td.BindFlags = D3D11_BIND_SHADER_RESOURCE | ((flags & BUI_TEX_TARGET) ? D3D11_BIND_RENDER_TARGET : 0);
  D3D11_SUBRESOURCE_DATA init = {data, (UINT)pitch, 0};
  if (FAILED(ID3D11Device_CreateTexture2D(d->dev, &td, data ? &init : NULL, &t->tex)) ||
      FAILED(ID3D11Device_CreateShaderResourceView(d->dev, (ID3D11Resource *)t->tex, NULL, &t->srv)) ||
      ((flags & BUI_TEX_TARGET) &&
       FAILED(ID3D11Device_CreateRenderTargetView(d->dev, (ID3D11Resource *)t->tex, NULL, &t->rtv))))
  {
    d11_tex_free(g, t);
    return NULL;
  }
  if (t->rtv) { /* a texture made without data is undefined: it is cleared */
    const float zero[4] = {0, 0, 0, 0};
    ID3D11DeviceContext_ClearRenderTargetView(d->ctx, t->rtv, zero);
  }
  return t;
}

static void d11_tex_update(BuiGpu *g, BuiGpuTex *t, int x, int y, int w, int h, const void *data,
                           int pitch)
{
  D3D11_BOX box = {(UINT)x, (UINT)y, 0, (UINT)(x + w), (UINT)(y + h), 1};
  ID3D11DeviceContext_UpdateSubresource(DEV(g)->ctx, (ID3D11Resource *)t->tex, 0, &box, data,
                                        (UINT)pitch, 0);
}

static void d11_fx_free(BuiGpu *g, BuiGpuProg *p)
{
  (void)g;
  REL(p->vs);
  REL(p->ps);
  REL(p->cb);
  free(p);
}

static BuiGpuProg *d11_fx_build(BuiGpu *g, const BuiEffectView *v, char *log, size_t log_n)
{
  D11 *d = DEV(g);
  ID3DBlob *vsb, *psb;
  if (!bui_dx_fx_compile(v, "4_0", &vsb, &psb, log, log_n)) {
    return NULL;
  }
  BuiGpuProg *p = (BuiGpuProg *)calloc(1, sizeof(BuiGpuProg));
  D3D11_BUFFER_DESC bd = {0};
  bd.ByteWidth = (UINT)(sizeof(float) * (8 + 4 * (size_t)(v->n_floats > 0 ? v->n_floats : 1)));
  bd.Usage = D3D11_USAGE_DYNAMIC;
  bd.BindFlags = D3D11_BIND_CONSTANT_BUFFER;
  bd.CPUAccessFlags = D3D11_CPU_ACCESS_WRITE;
  const int ok = p &&
                 SUCCEEDED(ID3D11Device_CreateVertexShader(d->dev, ID3D10Blob_GetBufferPointer(vsb),
                                                           ID3D10Blob_GetBufferSize(vsb), NULL,
                                                           &p->vs)) &&
                 SUCCEEDED(ID3D11Device_CreatePixelShader(d->dev, ID3D10Blob_GetBufferPointer(psb),
                                                          ID3D10Blob_GetBufferSize(psb), NULL,
                                                          &p->ps)) &&
                 SUCCEEDED(ID3D11Device_CreateBuffer(d->dev, &bd, NULL, &p->cb));
  REL(vsb);
  REL(psb);
  if (!ok) {
    snprintf(log, log_n, "the device rejected the compiled shader");
    if (p) {
      d11_fx_free(g, p);
    }
    return NULL;
  }
  return p;
}

/** \} */

/* -------------------------------------------------------------------- */
/** \name Drawing + present
 * \{ */

static int d11_target(BuiGpu *g, int tw, int th)
{
  D11 *d = DEV(g);
  if (!d->target || d->tw != tw || d->th != th) {
    REL(d->rtv);
    REL(d->target);
    D3D11_TEXTURE2D_DESC td = {0};
    td.Width = (UINT)tw;
    td.Height = (UINT)th;
    td.MipLevels = td.ArraySize = 1;
    td.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
    td.SampleDesc.Count = 1;
    td.Usage = D3D11_USAGE_DEFAULT;
    td.BindFlags = D3D11_BIND_RENDER_TARGET;
    if (FAILED(ID3D11Device_CreateTexture2D(d->dev, &td, NULL, &d->target)) ||
        FAILED(ID3D11Device_CreateRenderTargetView(d->dev, (ID3D11Resource *)d->target, NULL,
                                                   &d->rtv)))
    {
      REL(d->target);
      return 0;
    }
    d->tw = tw;
    d->th = th;
  }
  const float clear[4] = {0, 0, 0, g->alpha ? 0.0f : 1.0f};
  ID3D11DeviceContext_ClearRenderTargetView(d->ctx, d->rtv, clear);
  return 1;
}

static int ensure_buffer(D11 *d, ID3D11Buffer **buf, UINT *cap, UINT need, UINT stride, UINT bind)
{
  if (*buf && *cap >= need) {
    return 1;
  }
  REL(*buf);
  UINT c = *cap ? *cap : 4096;
  while (c < need) {
    c *= 2;
  }
  D3D11_BUFFER_DESC bd = {0};
  bd.ByteWidth = c * stride;
  bd.Usage = D3D11_USAGE_DYNAMIC;
  bd.BindFlags = bind;
  bd.CPUAccessFlags = D3D11_CPU_ACCESS_WRITE;
  if (FAILED(ID3D11Device_CreateBuffer(d->dev, &bd, NULL, buf))) {
    *cap = 0;
    return 0;
  }
  *cap = c;
  return 1;
}

static void upload(D11 *d, ID3D11Buffer *buf, const void *data, size_t bytes)
{
  D3D11_MAPPED_SUBRESOURCE m;
  if (SUCCEEDED(ID3D11DeviceContext_Map(d->ctx, (ID3D11Resource *)buf, 0, D3D11_MAP_WRITE_DISCARD,
                                        0, &m)))
  {
    memcpy(m.pData, data, bytes);
    ID3D11DeviceContext_Unmap(d->ctx, (ID3D11Resource *)buf, 0);
  }
}

/* One recorded effect draw: its shaders, floats and inputs, scissored to its clip. */
static void fx_draw(BuiGpu *g, const BuiGpuFxCall *fc)
{
  D11 *d = DEV(g);
  ID3D11DeviceContext *c = d->ctx;
  const float blend_factor[4] = {0, 0, 0, 0};
  /* a pass into a render target: its view and size stand in for the frame's until it is drawn */
  const BuiGpuDevTex *dst = fc->dst ? bui_gpu_dtex(g, fc->dst) : NULL;
  if (fc->dst && (!dst || !dst->tex->rtv)) {
    return;
  }
  if (!fc->effect) { /* a clear of the target */
    ID3D11DeviceContext_ClearRenderTargetView(c, dst->tex->rtv, g->fxf + fc->floats);
    return;
  }
  const BuiGpuDevFx *f = bui_gpu_dfx(g, fc->effect);
  if (!f) {
    return;
  }
  BuiGpuFxConsts consts;
  upload(d, f->prog->cb, consts.data,
         bui_gpu_fx_consts(g, fc, f->n_floats, dst ? dst->w : d->tw, dst ? dst->h : d->th, &consts));
  ID3D11ShaderResourceView *srv[BUI_EFFECT_MAX_INPUTS];
  for (int k = 0; k < f->n_inputs; k++) {
    srv[k] = bui_gpu_dtex_tex(g, fc->inputs[k])->srv;
  }
  if (dst) {
    /* nothing the last draw bound may be the target about to be written */
    ID3D11ShaderResourceView *none[BUI_EFFECT_MAX_INPUTS] = {0};
    ID3D11DeviceContext_PSSetShaderResources(c, 8, BUI_EFFECT_MAX_INPUTS, none);
    ID3D11DeviceContext_OMSetRenderTargets(c, 1, (ID3D11RenderTargetView *const *)&dst->tex->rtv,
                                           NULL);
    ID3D11DeviceContext_OMSetBlendState(c, NULL, blend_factor, 0xffffffffu);
    const D3D11_VIEWPORT vp = {0, 0, (float)dst->w, (float)dst->h, 0, 1};
    ID3D11DeviceContext_RSSetViewports(c, 1, &vp);
  }
  if (f->n_inputs > 0) {
    ID3D11DeviceContext_PSSetShaderResources(c, 8, (UINT)f->n_inputs, srv);
  }
  ID3D11DeviceContext_VSSetShader(c, f->prog->vs, NULL, 0);
  ID3D11DeviceContext_PSSetShader(c, f->prog->ps, NULL, 0);
  ID3D11DeviceContext_VSSetConstantBuffers(c, 1, 1, (ID3D11Buffer *const *)&f->prog->cb);
  ID3D11DeviceContext_PSSetConstantBuffers(c, 1, 1, (ID3D11Buffer *const *)&f->prog->cb);
  const D3D11_RECT sr = {fc->clip[0], fc->clip[1], fc->clip[2], fc->clip[3]};
  ID3D11DeviceContext_RSSetScissorRects(c, 1, &sr);
  ID3D11DeviceContext_Draw(c, 4, 0);
  if (dst) { /* back to the frame: its view, its size and its blending */
    ID3D11ShaderResourceView *none[BUI_EFFECT_MAX_INPUTS] = {0};
    ID3D11DeviceContext_PSSetShaderResources(c, 8, BUI_EFFECT_MAX_INPUTS, none);
    ID3D11DeviceContext_OMSetRenderTargets(c, 1, &d->rtv, NULL);
    ID3D11DeviceContext_OMSetBlendState(c, d->blend, blend_factor, 0xffffffffu);
    const D3D11_VIEWPORT vp = {0, 0, (float)d->tw, (float)d->th, 0, 1};
    ID3D11DeviceContext_RSSetViewports(c, 1, &vp);
  }
}

static ID3D11ShaderResourceView *srv_of(const BuiGpuTex *t)
{
  return t ? t->srv : NULL;
}

static void d11_draw(BuiGpu *g)
{
  D11 *d = DEV(g);
  ID3D11DeviceContext *c = d->ctx;
  for (int k = 0; k < BUI_DX_PROGRAMS; k++) {
    int n;
    const void *data = bui_gpu_batch_data(g, (BuiGpuBatchKind)k, &n);
    const UINT stride = bui_dx_programs[k].stride;
    if (n && ensure_buffer(d, &d->vb[k], &d->vb_cap[k], (UINT)n, stride, D3D11_BIND_VERTEX_BUFFER)) {
      upload(d, d->vb[k], data, (size_t)stride * n);
    }
  }
  if (g->n_mi && ensure_buffer(d, &d->mib, &d->mib_cap, (UINT)g->n_mi, sizeof(uint32_t),
                               D3D11_BIND_INDEX_BUFFER))
  {
    upload(d, d->mib, g->mi, sizeof(uint32_t) * (size_t)g->n_mi);
  }
  const float cbd[4] = {2.0f / (float)d->tw, 2.0f / (float)d->th, 0, 0};
  upload(d, d->cb, cbd, sizeof(cbd));

  ID3D11DeviceContext_OMSetRenderTargets(c, 1, &d->rtv, NULL);
  const float bf[4] = {0, 0, 0, 0};
  ID3D11DeviceContext_OMSetBlendState(c, d->blend, bf, 0xffffffffu);
  D3D11_VIEWPORT vp = {0, 0, (float)d->tw, (float)d->th, 0, 1};
  ID3D11DeviceContext_RSSetViewports(c, 1, &vp);
  ID3D11DeviceContext_RSSetState(c, d->rs);
  ID3D11DeviceContext_VSSetConstantBuffers(c, 0, 1, &d->cb);
  ID3D11ShaderResourceView *srv[4] = {srv_of(g->mask.tex), srv_of(g->rgba.tex),
                                      srv_of(g->curve_tex), srv_of(g->band_tex)};
  ID3D11DeviceContext_PSSetShaderResources(c, 0, 4, srv);

  int cur = -1;
  const BuiGpuTex *img = g->rgba.tex;
  for (int i = 0; i < g->n_b; i++) {
    const BuiGpuBatch *b = &g->b[i];
    if (b->count == 0) {
      continue;
    }
    if ((int)b->kind != cur) {
      if (cur == BUI_BATCH_MESH || cur == BUI_BATCH_FX) { /* the scissored states */
        ID3D11DeviceContext_RSSetState(c, d->rs);
      }
      cur = (int)b->kind;
      /* quads: 4 vertices from SV_VertexID per instance, as effects (which have no vertex data) */
      ID3D11DeviceContext_IASetPrimitiveTopology(c, b->kind == BUI_BATCH_QUADS || b->kind == BUI_BATCH_FX ?
                                                        D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP :
                                                        D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
      if (b->kind == BUI_BATCH_FX) {
        ID3D11DeviceContext_IASetInputLayout(c, NULL);
        ID3D11DeviceContext_PSSetSamplers(c, 0, 1, &d->smp_linear);
        ID3D11DeviceContext_RSSetState(c, d->rs_scissor);
      }
      else {
        const UINT stride = bui_dx_programs[b->kind].stride, off = 0;
        ID3D11DeviceContext_IASetInputLayout(c, d->il[b->kind]);
        ID3D11DeviceContext_IASetVertexBuffers(c, 0, 1, &d->vb[b->kind], &stride, &off);
        ID3D11DeviceContext_VSSetShader(c, d->vs[b->kind], NULL, 0);
        ID3D11DeviceContext_PSSetShader(c, d->ps[b->kind], NULL, 0);
      }
      if (b->kind == BUI_BATCH_MESH) {
        ID3D11DeviceContext_IASetIndexBuffer(c, d->mib, DXGI_FORMAT_R32_UINT, 0);
        ID3D11DeviceContext_RSSetState(c, d->rs_scissor);
      }
    }
    if (b->kind == BUI_BATCH_MESH) {
      ID3D11ShaderResourceView *ts = bui_gpu_dtex_tex(g, b->tex)->srv;
      ID3D11DeviceContext_PSSetShaderResources(c, 4, 1, &ts);
      ID3D11DeviceContext_PSSetSamplers(c, 0, 1, b->nearest ? &d->smp_point : &d->smp_linear);
      const D3D11_RECT sr = {b->clip[0], b->clip[1], b->clip[2], b->clip[3]};
      ID3D11DeviceContext_RSSetScissorRects(c, 1, &sr);
      ID3D11DeviceContext_DrawIndexed(c, (UINT)b->count, (UINT)b->first, 0);
    }
    else if (b->kind == BUI_BATCH_QUADS) {
      BuiGpuTex *uv;
      BuiGpuTex *want = bui_gpu_batch_image(g, b, &uv);
      if (want != img) { /* t1: the atlas, or a big image's own texture (t5: NV12 chroma) */
        img = want;
        ID3D11ShaderResourceView *t1 = srv_of(img), *t5 = srv_of(uv);
        ID3D11DeviceContext_PSSetShaderResources(c, 1, 1, &t1);
        ID3D11DeviceContext_PSSetShaderResources(c, 5, 1, &t5);
      }
      ID3D11DeviceContext_DrawInstanced(c, 4, (UINT)b->count, 0, (UINT)b->first);
    }
    else if (b->kind == BUI_BATCH_FX) {
      fx_draw(g, &g->fx[b->first]);
    }
    else {
      ID3D11DeviceContext_Draw(c, (UINT)b->count, (UINT)b->first);
    }
  }
  if (cur == BUI_BATCH_MESH || cur == BUI_BATCH_FX) {
    ID3D11DeviceContext_RSSetState(c, d->rs);
  }
}

static int d11_swap(BuiGpu *g, int cw, int ch)
{
  D11 *d = DEV(g);
  if (d->swap) {
    ID3D11DeviceContext_OMSetRenderTargets(d->ctx, 0, NULL, NULL);
    return SUCCEEDED(IDXGISwapChain1_ResizeBuffers(d->swap, 0, (UINT)cw, (UINT)ch,
                                                   DXGI_FORMAT_UNKNOWN, 0));
  }
  DXGI_SWAP_CHAIN_DESC1 sd = bui_dx_swap_desc(cw, ch);
  if (g->alpha) {
    /* bitblt model, as SDL's transparent D3D11 windows: Present copies the alpha channel into
     * the window's surface, which DWM blends (the host enables blur-behind) */
    sd.SwapEffect = DXGI_SWAP_EFFECT_DISCARD;
    sd.Scaling = DXGI_SCALING_STRETCH;
    sd.AlphaMode = DXGI_ALPHA_MODE_UNSPECIFIED;
  }
  if (FAILED(IDXGIFactory2_CreateSwapChainForHwnd(d->factory, (IUnknown *)d->dev, g->hwnd, &sd,
                                                  NULL, NULL, &d->swap)))
  {
    return 0;
  }
  IDXGIFactory2_MakeWindowAssociation(d->factory, g->hwnd, DXGI_MWA_NO_ALT_ENTER);
  return 1;
}

static void d11_clear(BuiGpu *g, const RECT *r)
{
  D11 *d = DEV(g);
  const float clear[4] = {0, 0, 0, 0};
  ID3D11DeviceContext1_ClearView(d->ctx1, (ID3D11View *)d->rtv, clear, r, 1);
}

static void copy_rect(D11 *d, ID3D11Texture2D *dst, const D3D11_BOX *b)
{
  ID3D11DeviceContext_CopySubresourceRegion(d->ctx, (ID3D11Resource *)dst, 0, b->left, b->top, 0,
                                            (ID3D11Resource *)d->target, 0, b);
}

static int d11_present(BuiGpu *g, int whole, const RECT *copy, int n_copy, const RECT *dirty,
                       int n_dirty)
{
  D11 *d = DEV(g);
  ID3D11Texture2D *back = NULL;
  if (FAILED(IDXGISwapChain1_GetBuffer(d->swap, 0, &k_IID_ID3D11Texture2D, (void **)&back))) {
    return BUI_PRESENT_LOST;
  }
  if (whole) {
    /* the target can be larger than the window (it grows in steps): copy the frame, not the resource */
    const D3D11_BOX frame = {0, 0, 0, (UINT)g->w, (UINT)g->h, 1};
    copy_rect(d, back, &frame);
  }
  for (int i = 0; i < n_copy; i++) {
    const D3D11_BOX b = {(UINT)copy[i].left, (UINT)copy[i].top, 0, (UINT)copy[i].right,
                         (UINT)copy[i].bottom, 1};
    copy_rect(d, back, &b);
  }
  REL(back);
  DXGI_PRESENT_PARAMETERS pp = {0};
  pp.DirtyRectsCount = (UINT)n_dirty;
  pp.pDirtyRects = n_dirty ? (RECT *)dirty : NULL;
  return bui_dx_present_status(IDXGISwapChain1_Present1(d->swap, 0, 0, &pp));
}

static int d11_read(BuiGpu *g, uint32_t *dst, int w, int h)
{
  D11 *d = DEV(g);
  D3D11_TEXTURE2D_DESC td = {0};
  td.Width = (UINT)w;
  td.Height = (UINT)h;
  td.MipLevels = td.ArraySize = 1;
  td.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
  td.SampleDesc.Count = 1;
  td.Usage = D3D11_USAGE_STAGING;
  td.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
  ID3D11Texture2D *st = NULL;
  if (FAILED(ID3D11Device_CreateTexture2D(d->dev, &td, NULL, &st))) {
    return 0;
  }
  const D3D11_BOX frame = {0, 0, 0, (UINT)w, (UINT)h, 1};
  ID3D11DeviceContext_CopySubresourceRegion(d->ctx, (ID3D11Resource *)st, 0, 0, 0, 0,
                                            (ID3D11Resource *)d->target, 0, &frame);
  D3D11_MAPPED_SUBRESOURCE m;
  int ok = 0;
  if (SUCCEEDED(ID3D11DeviceContext_Map(d->ctx, (ID3D11Resource *)st, 0, D3D11_MAP_READ, 0, &m))) {
    for (int y = 0; y < h; y++) {
      memcpy(dst + (size_t)y * w, (const uint8_t *)m.pData + (size_t)y * m.RowPitch,
             sizeof(uint32_t) * (size_t)w);
    }
    ID3D11DeviceContext_Unmap(d->ctx, (ID3D11Resource *)st, 0);
    ok = 1;
  }
  REL(st);
  return ok;
}

/** \} */

const BuiGpuOps bui_gpu_d3d11_ops = {
    BUI_GPU_API_D3D11,
    d11_init,
    d11_exit,
    d11_tex_create,
    d11_tex_update,
    d11_tex_free,
    d11_fx_build,
    d11_fx_free,
    d11_target,
    d11_draw,
    d11_clear,
    d11_swap,
    d11_present,
    NULL, /* submit: the immediate context sends work on its own */
    d11_read,
};

#endif /* _WIN32 && BUI_GPU */
