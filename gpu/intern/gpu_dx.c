/* What the Direct3D backends share — see gpu_dx.h.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */

#if defined(_WIN32) && defined(BUI_GPU) /* BUI_GPU: features.json */

#  define COBJMACROS
#  include "gpu_dx.h"

#  include <stdio.h>
#  include <stdlib.h>
#  include <string.h>

/* defined here rather than taken from dxguid.lib: nothing to link */
const GUID bui_IID_IDXGIFactory2 = {
    0x50c83a1c, 0xe072, 0x4c48, {0x87, 0xb0, 0x36, 0x30, 0xfa, 0x36, 0xa6, 0xd0}};
const GUID bui_IID_IDXGIFactory4 = {
    0x1bc6ea02, 0xef36, 0x464f, {0xbf, 0x0c, 0x21, 0xca, 0x39, 0xe5, 0x16, 0x8a}};
const GUID bui_IID_IDXGIAdapter = {
    0x2411e7e1, 0x12ac, 0x4ccf, {0xbd, 0x14, 0x97, 0x98, 0xe8, 0x53, 0x4d, 0xc0}};
const GUID bui_IID_IDXGIAdapter1 = {
    0x29038f61, 0x3839, 0x4626, {0x91, 0xfd, 0x08, 0x68, 0x79, 0x01, 0x1a, 0x05}};
static const GUID k_IID_IDXGIFactory6 = {
    0xc1b6694f, 0xff09, 0x44a9, {0xb0, 0x3c, 0x77, 0x90, 0x0a, 0x0a, 0x1d, 0x17}};

IDXGIAdapter *bui_dx_preferred_adapter(void)
{
  if (BUI_gpu_adapter() != BUI_GPU_ADAPTER_HIGH_PERFORMANCE) {
    return NULL; /* the display's adapter, the first DXGI lists */
  }
  typedef HRESULT(WINAPI * FactoryFn)(REFIID, void **);
  HMODULE dxgi = LoadLibraryW(L"dxgi.dll");
  FactoryFn make = dxgi ? (FactoryFn)(void *)GetProcAddress(dxgi, "CreateDXGIFactory1") : NULL;
  IDXGIFactory6 *f6 = NULL;
  IDXGIAdapter *ad = NULL;
  if (!make || FAILED(make(&k_IID_IDXGIFactory6, (void **)&f6))) {
    bui_gpu_fail("IDXGIFactory6 (Windows 10 1803+)", 0);
    return NULL;
  }
  if (FAILED(IDXGIFactory6_EnumAdapterByGpuPreference(f6, 0, DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE,
                                                      &bui_IID_IDXGIAdapter, (void **)&ad)))
  {
    ad = NULL;
  }
  REL(f6);
  return ad;
}

void bui_dx_report(const char *api, IDXGIAdapter *ad)
{
  DXGI_ADAPTER_DESC desc;
  if (bui_gpu_debug() && ad && SUCCEEDED(IDXGIAdapter_GetDesc(ad, &desc))) {
    fprintf(stderr, "bui gpu: %s on %ls\n", api, desc.Description);
  }
}

int bui_dx_present_status(HRESULT hr)
{
  if (hr == DXGI_ERROR_DEVICE_REMOVED || hr == DXGI_ERROR_DEVICE_RESET) {
    return BUI_PRESENT_LOST;
  }
  /* occluded or minimized: DWM dropped the frame, so the next one must be whole */
  return hr == DXGI_STATUS_OCCLUDED ? BUI_PRESENT_OCCLUDED : BUI_PRESENT_OK;
}

DXGI_SWAP_CHAIN_DESC1 bui_dx_swap_desc(int w, int h)
{
  DXGI_SWAP_CHAIN_DESC1 sd;
  memset(&sd, 0, sizeof(sd));
  sd.Width = (UINT)w;
  sd.Height = (UINT)h;
  sd.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
  sd.SampleDesc.Count = 1;
  sd.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
  sd.BufferCount = 2;
  sd.SwapEffect = DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL;
  sd.Scaling = DXGI_SCALING_NONE;
  sd.AlphaMode = DXGI_ALPHA_MODE_IGNORE;
  return sd;
}

/* -------------------------------------------------------------------- */
/** \name The renderer's programs
 * \{ */

#  include "gpu_d3d11_shaders.h"

#  define INST(name, idx, off) {name, idx, DXGI_FORMAT_R32G32B32A32_FLOAT, off, 1}
static const BuiDxInput k_quad[] = {
    INST("RECT", 0, 0),   INST("BOUNDS", 0, 16), INST("COLOR", 0, 32),  INST("COLOR", 1, 48),
    INST("COLOR", 2, 64), INST("COLOR", 3, 80),  INST("PARAMS", 0, 96), INST("EXTRA", 0, 112),
};
#  undef INST
static const BuiDxInput k_tri[] = {
    {"POSITION", 0, DXGI_FORMAT_R32G32_FLOAT, 0, 0},
    {"COLOR", 0, DXGI_FORMAT_R32G32B32A32_FLOAT, 8, 0},
    {"CLIP", 0, DXGI_FORMAT_R32G32B32A32_FLOAT, 24, 0},
};
static const BuiDxInput k_mesh[] = { /* BuiMeshVertex (= ImDrawVert) */
    {"POSITION", 0, DXGI_FORMAT_R32G32_FLOAT, 0, 0},
    {"TEXCOORD", 0, DXGI_FORMAT_R32G32_FLOAT, 8, 0},
    {"COLOR", 0, DXGI_FORMAT_R8G8B8A8_UNORM, 16, 0},
};

const BuiDxProgram bui_dx_programs[BUI_DX_PROGRAMS] = {
    {g_vs_quad, g_ps_quad, sizeof(g_vs_quad), sizeof(g_ps_quad), k_quad, 8, sizeof(BuiGpuInst)},
    {g_vs_tri, g_ps_tri, sizeof(g_vs_tri), sizeof(g_ps_tri), k_tri, 3, sizeof(BuiGpuTriV)},
    {g_vs_mesh, g_ps_mesh, sizeof(g_vs_mesh), sizeof(g_ps_mesh), k_mesh, 3, sizeof(BuiMeshVertex)},
};

/** \} */

/* -------------------------------------------------------------------- */
/** \name Effects: the HLSL compiler
 * \{ */

typedef HRESULT(WINAPI *D3DCompileFn)(LPCVOID, SIZE_T, LPCSTR, const D3D_SHADER_MACRO *,
                                      ID3DInclude *, LPCSTR, LPCSTR, UINT, UINT, ID3DBlob **,
                                      ID3DBlob **);

/* d3dcompiler_47.dll (Windows 8.1 and later ship it) is loaded when the first effect is drawn:
 * nothing links against it and no SDK is needed. System folder first, then the app's own. */
static D3DCompileFn g_d3d_compile;

static BOOL CALLBACK d3d_compiler_load(PINIT_ONCE once, PVOID param, PVOID *ctx)
{
  (void)once;
  (void)param;
  (void)ctx;
  HMODULE m = LoadLibraryExW(L"d3dcompiler_47.dll", NULL, LOAD_LIBRARY_SEARCH_SYSTEM32);
  if (!m) {
    m = LoadLibraryExW(L"d3dcompiler_47.dll", NULL, LOAD_LIBRARY_SEARCH_APPLICATION_DIR);
  }
  if (m) {
    g_d3d_compile = (D3DCompileFn)(void *)GetProcAddress(m, "D3DCompile");
  }
  return TRUE;
}

/* Loaded once, whichever thread draws the first effect (devices of several windows or tests
 * compile at the same time): another thread waits for the load rather than seeing it unfinished. */
static D3DCompileFn d3d_compiler(void)
{
  static INIT_ONCE once = INIT_ONCE_STATIC_INIT;
  InitOnceExecuteOnce(&once, d3d_compiler_load, NULL, NULL);
  return g_d3d_compile;
}

/* One stage of the assembled source; on failure the compiler's text goes to `log`. */
static ID3DBlob *fx_stage(D3DCompileFn compile, const char *src, const char *name,
                          const char *entry, const char *target, char *log, size_t log_n)
{
  ID3DBlob *code = NULL, *err = NULL;
  const HRESULT hr = compile(src, strlen(src), name, NULL, NULL, entry, target,
                             (1u << 15) /* D3DCOMPILE_OPTIMIZATION_LEVEL3 */, 0, &code, &err);
  if (FAILED(hr)) {
    snprintf(log, log_n, "%.*s",
             err ? (int)ID3D10Blob_GetBufferSize(err) : 0,
             err ? (const char *)ID3D10Blob_GetBufferPointer(err) : "compile failed");
    REL(code);
  }
  REL(err);
  return FAILED(hr) ? NULL : code;
}

int bui_dx_fx_compile(const BuiEffectView *v, const char *model, ID3DBlob **r_vs,
                      ID3DBlob **r_ps, char *log, size_t log_n)
{
  *r_vs = *r_ps = NULL;
  D3DCompileFn compile = d3d_compiler();
  if (!compile) {
    snprintf(log, log_n, "d3dcompiler_47.dll is not available: the effect cannot run on this system");
    return 0;
  }
  char *src = bui_effect_assemble(BUI_FX_HLSL, v);
  if (!src) {
    snprintf(log, log_n, "out of memory");
    return 0;
  }
  char ps[8], vs[8];
  snprintf(ps, sizeof(ps), "ps_%s", model);
  snprintf(vs, sizeof(vs), "vs_%s", model);
  *r_ps = fx_stage(compile, src, v->name, "bui_ps", ps, log, log_n);
  free(src);
  *r_vs = *r_ps ? fx_stage(compile, bui_fx_hlsl_vs, "effect_vs", "bui_vs", vs, log, log_n) : NULL;
  if (!*r_vs) {
    REL(*r_ps);
    return 0;
  }
  return 1;
}

/** \} */

#endif /* _WIN32 && BUI_GPU */
