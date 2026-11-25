/* HEIC on macOS (format_heic.c): ImageIO, the system's decoder (macOS 10.13 and newer read
 * HEIC; the framework is weakly linked, as AVFoundation is for video). The primary image is
 * decoded with its orientation applied and at the size drawn (an ImageIO thumbnail of the whole
 * image, CGImageSourceCreateThumbnailAtIndex with the transform), then drawn into the BKImBuf as
 * premultiplied BGRA, sRGB.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */

#ifdef BUI_HEIC_IMAGEIO

#  include <pthread.h>

#  include <CoreFoundation/CoreFoundation.h>
#  include <CoreGraphics/CoreGraphics.h>
#  include <ImageIO/ImageIO.h>

#  include "image.h"

static struct {
  pthread_once_t once;
  bool ok;
} IIO = {PTHREAD_ONCE_INIT};

static void iio_load(void)
{
  if (&CGImageSourceCopyTypeIdentifiers == NULL) {
    return; /* weakly linked ImageIO missing */
  }
  CFArrayRef types = CGImageSourceCopyTypeIdentifiers();
  if (types) {
    IIO.ok = CFArrayContainsValue(types, CFRangeMake(0, CFArrayGetCount(types)),
                                  CFSTR("public.heic"));
    CFRelease(types);
  }
}

bool imb_heic_supported(void)
{
  pthread_once(&IIO.once, iio_load);
  return IIO.ok;
}

/* A source on `mem` (kept by the caller while it is open), or NULL. */
static CGImageSourceRef iio_open(const uint8_t *mem, size_t size)
{
  if (!imb_heic_supported()) {
    return NULL;
  }
  CFDataRef data = CFDataCreateWithBytesNoCopy(NULL, mem, (CFIndex)size, kCFAllocatorNull);
  CGImageSourceRef src = data ? CGImageSourceCreateWithData(data, NULL) : NULL;
  if (data) {
    CFRelease(data); /* the source keeps it */
  }
  return src;
}

static int iio_int(CFDictionaryRef props, CFStringRef key, int fallback)
{
  int v = fallback;
  CFNumberRef n = (CFNumberRef)CFDictionaryGetValue(props, key);
  if (!n || CFGetTypeID(n) != CFNumberGetTypeID() || !CFNumberGetValue(n, kCFNumberIntType, &v)) {
    v = fallback;
  }
  return v;
}

/* The shown size (after the orientation) of the primary image. */
static bool iio_size(CGImageSourceRef src, int *r_w, int *r_h)
{
  CFDictionaryRef props = CGImageSourceCopyPropertiesAtIndex(src, 0, NULL);
  if (!props) {
    return false;
  }
  const int w = iio_int(props, kCGImagePropertyPixelWidth, 0);
  const int h = iio_int(props, kCGImagePropertyPixelHeight, 0);
  const bool swap = iio_int(props, kCGImagePropertyOrientation, 1) >= 5;
  CFRelease(props);
  if (w <= 0 || h <= 0 || w > BK_IMB_MAX_SIDE || h > BK_IMB_MAX_SIDE) {
    return false;
  }
  *r_w = swap ? h : w;
  *r_h = swap ? w : h;
  return true;
}

bool imb_header_heic(const uint8_t *mem, size_t size, BKImageInfo *r_info)
{
  CGImageSourceRef src = iio_open(mem, size);
  const bool ok = src && iio_size(src, &r_info->width, &r_info->height);
  if (src) {
    CFRelease(src);
  }
  r_info->frames = 1;
  return ok;
}

BKImBuf *imb_load_heic(const uint8_t *mem, size_t size, int want_w, int want_h)
{
  CGImageSourceRef src = iio_open(mem, size);
  int w, h;
  if (!src || !iio_size(src, &w, &h)) {
    if (src) {
      CFRelease(src);
    }
    return NULL;
  }
  /* the longer side drawn: the whole image at that scale, oriented */
  int side = BK_MAX(w, h);
  if (want_w > 0 && want_h > 0 && want_w < w && want_h < h) {
    side = BK_MAX(want_w, want_h);
  }
  CFNumberRef max = CFNumberCreate(NULL, kCFNumberIntType, &side);
  const void *keys[] = {kCGImageSourceCreateThumbnailFromImageAlways,
                        kCGImageSourceCreateThumbnailWithTransform,
                        kCGImageSourceThumbnailMaxPixelSize};
  const void *values[] = {kCFBooleanTrue, kCFBooleanTrue, max};
  CFDictionaryRef opts = CFDictionaryCreate(NULL, keys, values, 3, &kCFTypeDictionaryKeyCallBacks,
                                            &kCFTypeDictionaryValueCallBacks);
  CGImageRef img = CGImageSourceCreateThumbnailAtIndex(src, 0, opts);
  CFRelease(opts);
  CFRelease(max);
  CFRelease(src);
  BKImBuf *ib = NULL;
  if (img) {
    const size_t iw = CGImageGetWidth(img), ih = CGImageGetHeight(img);
    ib = iw > 0 && ih > 0 && iw <= BK_IMB_MAX_SIDE && ih <= BK_IMB_MAX_SIDE ?
             imb_alloc((int)iw, (int)ih, 1) :
             NULL;
    CGColorSpaceRef cs = ib ? CGColorSpaceCreateWithName(kCGColorSpaceSRGB) : NULL;
    CGContextRef ctx = cs ? CGBitmapContextCreate(ib->rect, iw, ih, 8, iw * 4, cs,
                                                  kCGImageAlphaPremultipliedFirst |
                                                      kCGBitmapByteOrder32Little) :
                            NULL;
    if (ctx) {
      CGContextSetBlendMode(ctx, kCGBlendModeCopy);
      CGContextDrawImage(ctx, CGRectMake(0, 0, (CGFloat)iw, (CGFloat)ih), img);
      CGContextRelease(ctx);
    }
    else if (ib) {
      BK_imbuf_free(ib);
      ib = NULL;
    }
    if (cs) {
      CGColorSpaceRelease(cs);
    }
    CGImageRelease(img);
  }
  return ib;
}

#endif /* BUI_HEIC_IMAGEIO */
