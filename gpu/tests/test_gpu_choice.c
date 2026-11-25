/* The app's GPU choices and their environment overrides: BUI_gpu_set_api / BUI_GPU_API and
 * BUI_gpu_set_adapter / BUI_GPU_ADAPTER. No device is made, so this runs everywhere.
 * SPDX-License-Identifier: GPL-2.0-or-later */
#include "GPU_raster.h"

#include <stdio.h>
#include <stdlib.h>

static int failures = 0;

static void check(const char *what, int ok)
{
  if (!ok) {
    printf("gpu choice: %s failed\n", what);
    failures++;
  }
}

/* value NULL unsets the variable */
static void set_env(const char *name, const char *value)
{
#ifdef _WIN32
  _putenv_s(name, value ? value : "");
#else
  if (value) {
    setenv(name, value, 1);
  }
  else {
    unsetenv(name);
  }
#endif
}

int main(void)
{
  set_env("BUI_GPU_API", NULL);
  set_env("BUI_GPU_ADAPTER", NULL);

  /* the adapter: the display's until the app or the environment asks for another */
  check("adapter default", BUI_gpu_adapter() == BUI_GPU_ADAPTER_DEFAULT);
  BUI_gpu_set_adapter(BUI_GPU_ADAPTER_HIGH_PERFORMANCE);
  check("adapter set by the app", BUI_gpu_adapter() == BUI_GPU_ADAPTER_HIGH_PERFORMANCE);
  BUI_gpu_set_adapter((BuiGpuAdapter)7);
  check("adapter: an unknown value is ignored", BUI_gpu_adapter() == BUI_GPU_ADAPTER_HIGH_PERFORMANCE);
  set_env("BUI_GPU_ADAPTER", "default");
  check("BUI_GPU_ADAPTER=default overrides the app", BUI_gpu_adapter() == BUI_GPU_ADAPTER_DEFAULT);
  set_env("BUI_GPU_ADAPTER", "fastest");
  check("BUI_GPU_ADAPTER: a value it doesn't take falls back to the app's",
        BUI_gpu_adapter() == BUI_GPU_ADAPTER_HIGH_PERFORMANCE);
  BUI_gpu_set_adapter(BUI_GPU_ADAPTER_DEFAULT);
  set_env("BUI_GPU_ADAPTER", "high-performance");
  check("BUI_GPU_ADAPTER=high-performance overrides the app",
        BUI_gpu_adapter() == BUI_GPU_ADAPTER_HIGH_PERFORMANCE);
  set_env("BUI_GPU_ADAPTER", NULL);
  check("adapter back to the app's", BUI_gpu_adapter() == BUI_GPU_ADAPTER_DEFAULT);

  /* the API, the same way (the baked default is Direct3D 11) */
  check("api default", BUI_gpu_api() == BUI_GPU_API_D3D11);
  BUI_gpu_set_api(BUI_GPU_API_D3D12);
  check("api set by the app", BUI_gpu_api() == BUI_GPU_API_D3D12);
  set_env("BUI_GPU_API", "d3d11");
  check("BUI_GPU_API overrides the app", BUI_gpu_api() == BUI_GPU_API_D3D11);
  set_env("BUI_GPU_API", "vulkan");
  check("BUI_GPU_API: a value it doesn't take falls back to the app's", BUI_gpu_api() == BUI_GPU_API_D3D12);
  set_env("BUI_GPU_API", NULL);
  BUI_gpu_set_api(BUI_GPU_API_DEFAULT);
  check("api back to the default", BUI_gpu_api() == BUI_GPU_API_D3D11);

  if (failures == 0) {
    printf("gpu choice: ok\n");
  }
  return failures == 0 ? 0 : 1;
}
