/* The GPU tests attach their device through this: it runs on the API BUI_gpu_api() names (the
 * BUI_GPU_API variable, which the *_d3d12 tests set), and one that fell back to another API ends
 * the run as a failure instead of passing on the wrong one.
 * SPDX-License-Identifier: GPL-2.0-or-later */
#pragma once

#include "GPU_raster.h"

#include <stdio.h>
#include <stdlib.h>

/* BUI_raster_gpu_attach(r, NULL, 1): 0 when there is no GPU device (the test skips). */
static int test_gpu_attach(BuiRaster *r)
{
  if (!BUI_raster_gpu_attach(r, NULL, 1)) {
    return 0;
  }
#ifdef _WIN32
  if (BUI_raster_gpu_api(r) != BUI_gpu_api()) {
    fprintf(stderr, "gpu test: asked for GPU API %d, the device runs on %d (BUI_GPU_DEBUG=1 says why)\n",
            (int)BUI_gpu_api(), (int)BUI_raster_gpu_api(r));
    exit(1);
  }
#endif
  return 1;
}
