/* HEIC on Windows (format_heic.c): the Windows Imaging Component's HEIF decoder, which the
 * "HEIF Image Extensions" install (with the "HEVC Video Extensions" for HEVC pictures): the
 * primary image as that decoder gives it, converted to premultiplied BGRA by WIC, and scaled by
 * WIC when smaller is wanted (IWICBitmapScaler, Fant: an area average).
 *
 * ole32.dll is loaded on first use, as Media Foundation is for video (movie_mf.c): nothing is
 * linked. Every call initializes COM on its thread (multithreaded apartment) for its duration,
 * and makes its own factory: decodes run on the kit's workers and on callers' threads.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */

#ifdef BUI_HEIC_WIC

#  define COBJMACROS
#  define WIN32_LEAN_AND_MEAN
#  include <windows.h>

#  include <wincodec.h>

#  include "MEM_guardedalloc.h"

#  include "image.h"

/* the GUIDs, here: nothing links uuid.lib or windowscodecs.lib */
static const GUID heic_clsid_factory = {
    0xcacaf262, 0x9370, 0x4615, {0xa1, 0x3b, 0x9f, 0x55, 0x39, 0xda, 0x4c, 0x0a}};
static const GUID heic_iid_factory = {
    0xec5ec8a9, 0xc395, 0x4314, {0x9c, 0x77, 0x54, 0xd7, 0xa9, 0x35, 0xff, 0x70}};
static const GUID heic_container = { /* GUID_ContainerFormatHeif */
    0xe1e62521, 0x6787, 0x405b, {0xa3, 0x39, 0x50, 0x07, 0x15, 0xb5, 0x76, 0x3f}};
static const GUID heic_pbgra = { /* GUID_WICPixelFormat32bppPBGRA */
    0x6fddc324, 0x4e03, 0x4bfe, {0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x10}};

typedef HRESULT(WINAPI *CoInitializeExFn)(LPVOID, DWORD);
typedef void(WINAPI *CoUninitializeFn)(void);
typedef HRESULT(WINAPI *CoCreateInstanceFn)(REFCLSID, LPUNKNOWN, DWORD, REFIID, LPVOID *);

static struct {
  INIT_ONCE once;
  bool ok; /* COM is there and WIC has a HEIF decoder */
  CoInitializeExFn co_init;
  CoUninitializeFn co_uninit;
  CoCreateInstanceFn co_create;
} WIC = {INIT_ONCE_STATIC_INIT};

/* COM on this thread for one call, and a factory (NULL: none). `*r_uninit`: undo the init. */
static IWICImagingFactory *wic_begin(bool *r_uninit)
{
  const HRESULT hr = WIC.co_init(NULL, COINIT_MULTITHREADED);
  *r_uninit = SUCCEEDED(hr); /* S_OK or S_FALSE: ours to balance; RPC_E_CHANGED_MODE: an STA's */
  IWICImagingFactory *f = NULL;
  if (FAILED(WIC.co_create(&heic_clsid_factory, NULL, CLSCTX_INPROC_SERVER, &heic_iid_factory,
                           (void **)&f)))
  {
    f = NULL;
  }
  return f;
}

static void wic_end(IWICImagingFactory *f, bool uninit)
{
  if (f) {
    IWICImagingFactory_Release(f);
  }
  if (uninit) {
    WIC.co_uninit();
  }
}

static BOOL CALLBACK wic_load(PINIT_ONCE once, PVOID param, PVOID *ctx)
{
  (void)once;
  (void)param;
  (void)ctx;
  HMODULE ole = LoadLibraryW(L"ole32.dll");
  WIC.co_init = ole ? (CoInitializeExFn)(void *)GetProcAddress(ole, "CoInitializeEx") : NULL;
  WIC.co_uninit = ole ? (CoUninitializeFn)(void *)GetProcAddress(ole, "CoUninitialize") : NULL;
  WIC.co_create = ole ? (CoCreateInstanceFn)(void *)GetProcAddress(ole, "CoCreateInstance") :
                        NULL;
  if (!WIC.co_init || !WIC.co_uninit || !WIC.co_create) {
    return TRUE;
  }
  bool uninit;
  IWICImagingFactory *f = wic_begin(&uninit);
  IWICBitmapDecoder *dec = NULL;
  WIC.ok = f && SUCCEEDED(IWICImagingFactory_CreateDecoder(f, &heic_container, NULL, &dec));
  if (dec) {
    IWICBitmapDecoder_Release(dec);
  }
  wic_end(f, uninit);
  return TRUE;
}

bool imb_heic_supported(void)
{
  InitOnceExecuteOnce(&WIC.once, wic_load, NULL, NULL);
  return WIC.ok;
}

/* The first frame of `mem` (NULL on failure); `*r_f`, `*r_uninit`: wic_end's. */
static IWICBitmapFrameDecode *wic_frame(const uint8_t *mem, size_t size, IWICImagingFactory **r_f,
                                        bool *r_uninit, IWICBitmapDecoder **r_dec,
                                        IWICStream **r_stream)
{
  *r_dec = NULL;
  *r_stream = NULL;
  *r_f = (imb_heic_supported() && size <= MAXDWORD) ? wic_begin(r_uninit) : NULL;
  if (!*r_f) {
    *r_uninit = false;
    return NULL;
  }
  IWICBitmapFrameDecode *frame = NULL;
  if (SUCCEEDED(IWICImagingFactory_CreateStream(*r_f, r_stream)) &&
      SUCCEEDED(IWICStream_InitializeFromMemory(*r_stream, (BYTE *)mem, (DWORD)size)) &&
      SUCCEEDED(IWICImagingFactory_CreateDecoderFromStream(*r_f, (IStream *)*r_stream, NULL,
                                                           WICDecodeMetadataCacheOnDemand, r_dec)))
  {
    if (FAILED(IWICBitmapDecoder_GetFrame(*r_dec, 0, &frame))) {
      frame = NULL;
    }
  }
  return frame;
}

static void wic_frame_end(IWICBitmapFrameDecode *frame, IWICImagingFactory *f, bool uninit,
                          IWICBitmapDecoder *dec, IWICStream *stream)
{
  if (frame) {
    IWICBitmapFrameDecode_Release(frame);
  }
  if (dec) {
    IWICBitmapDecoder_Release(dec);
  }
  if (stream) {
    IWICStream_Release(stream);
  }
  wic_end(f, uninit);
}

bool imb_header_heic(const uint8_t *mem, size_t size, BKImageInfo *r_info)
{
  IWICImagingFactory *f;
  IWICBitmapDecoder *dec;
  IWICStream *stream;
  bool uninit;
  IWICBitmapFrameDecode *frame = wic_frame(mem, size, &f, &uninit, &dec, &stream);
  UINT w = 0, h = 0;
  const bool ok = frame && SUCCEEDED(IWICBitmapFrameDecode_GetSize(frame, &w, &h)) && w > 0 &&
                  h > 0 && w <= BK_IMB_MAX_SIDE && h <= BK_IMB_MAX_SIDE;
  wic_frame_end(frame, f, uninit, dec, stream);
  r_info->width = (int)w;
  r_info->height = (int)h;
  r_info->frames = 1;
  return ok;
}

BKImBuf *imb_load_heic(const uint8_t *mem, size_t size, int want_w, int want_h)
{
  IWICImagingFactory *f;
  IWICBitmapDecoder *dec;
  IWICStream *stream;
  bool uninit;
  IWICBitmapFrameDecode *frame = wic_frame(mem, size, &f, &uninit, &dec, &stream);
  UINT w = 0, h = 0;
  BKImBuf *ib = NULL;
  if (frame && SUCCEEDED(IWICBitmapFrameDecode_GetSize(frame, &w, &h)) && w > 0 && h > 0 &&
      w <= BK_IMB_MAX_SIDE && h <= BK_IMB_MAX_SIDE)
  {
    IWICBitmapSource *src = (IWICBitmapSource *)frame;
    IWICBitmapScaler *scaler = NULL;
    if (want_w > 0 && want_h > 0 && (UINT)want_w < w && (UINT)want_h < h &&
        SUCCEEDED(IWICImagingFactory_CreateBitmapScaler(f, &scaler)) &&
        SUCCEEDED(IWICBitmapScaler_Initialize(scaler, src, (UINT)want_w, (UINT)want_h,
                                              WICBitmapInterpolationModeFant)))
    {
      src = (IWICBitmapSource *)scaler; /* the decoder's own scaling (Fant: area average) */
      w = (UINT)want_w;
      h = (UINT)want_h;
    }
    IWICFormatConverter *conv = NULL;
    ib = imb_alloc((int)w, (int)h, 1);
    if (ib && (FAILED(IWICImagingFactory_CreateFormatConverter(f, &conv)) ||
               FAILED(IWICFormatConverter_Initialize(conv, src, &heic_pbgra,
                                                     WICBitmapDitherTypeNone, NULL, 0.0,
                                                     WICBitmapPaletteTypeCustom)) ||
               FAILED(IWICFormatConverter_CopyPixels(conv, NULL, w * 4,
                                                     (UINT)(sizeof(uint32_t) * (size_t)w * h),
                                                     (BYTE *)ib->rect))))
    {
      BK_imbuf_free(ib); /* no HEVC decoder, or a broken file */
      ib = NULL;
    }
    if (conv) {
      IWICFormatConverter_Release(conv);
    }
    if (scaler) {
      IWICBitmapScaler_Release(scaler);
    }
  }
  wic_frame_end(frame, f, uninit, dec, stream);
  return ib;
}

#endif /* BUI_HEIC_WIC */
