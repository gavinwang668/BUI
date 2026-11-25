/* What the Direct3D backends (gpu_d3d11.c, gpu_d3d12.c) share: DXGI (the adapter, the swap chain's
 * present result) and the HLSL compiler for effects.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */
#pragma once

#include "gpu_device.h"

#include <d3dcommon.h>
#include <dxgi1_6.h>

#define REL(p) \
  do { \
    if (p) { \
      IUnknown_Release((IUnknown *)(p)); \
      (p) = NULL; \
    } \
  } while (0)

extern const GUID bui_IID_IDXGIFactory2;
extern const GUID bui_IID_IDXGIFactory4;
extern const GUID bui_IID_IDXGIAdapter;
extern const GUID bui_IID_IDXGIAdapter1;

/* The adapter `BUI_GPU_ADAPTER` asks for: "high-performance" is DXGI's choice for that preference
 * (the discrete GPU of a hybrid laptop, which Nsight and other vendor tools profile); unset or
 * "default" is NULL, the display's adapter Windows picks. The caller releases it. */
IDXGIAdapter *bui_dx_preferred_adapter(void);
/* BUI_GPU_DEBUG: which API and adapter the device runs on. */
void bui_dx_report(const char *api, IDXGIAdapter *ad);
/* What a swap chain's Present returned, as BUI_PRESENT_*. */
int bui_dx_present_status(HRESULT hr);
/* The flip-model swap chain of an opaque window: B8G8R8A8, two buffers (the damage of the last
 * present is redrawn into the buffer of the one before, gpu_device.c). */
DXGI_SWAP_CHAIN_DESC1 bui_dx_swap_desc(int w, int h);

/* The renderer's own programs (gpu/shaders/bui_ui.hlsl, compiled into gpu_d3d11_shaders.h as
 * shader model 4.0 bytecode, which both APIs take): one per batch kind that has vertex data. */
typedef struct BuiDxInput {
  const char *semantic;
  UINT index;
  DXGI_FORMAT format;
  UINT offset;
  UINT instanced; /* per instance (step rate 1), else per vertex */
} BuiDxInput;
typedef struct BuiDxProgram {
  const void *vs, *ps;
  size_t vs_size, ps_size;
  const BuiDxInput *inputs;
  UINT n_inputs;
  UINT stride; /* bytes per vertex (or instance) */
} BuiDxProgram;
/* by BuiGpuBatchKind: BUI_BATCH_QUADS, BUI_BATCH_TRIS, BUI_BATCH_MESH */
#define BUI_DX_PROGRAMS 3
extern const BuiDxProgram bui_dx_programs[BUI_DX_PROGRAMS];

/* Compiles effect `v` (gpu_effect.h) to vertex and pixel shader bytecode for shader model
 * `model` ("4_0", "5_0"). 0 with the compiler's text in `log` when it fails. */
int bui_dx_fx_compile(const BuiEffectView *v, const char *model, ID3DBlob **r_vs,
                      ID3DBlob **r_ps, char *log, size_t log_n);
