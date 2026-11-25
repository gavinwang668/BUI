/* BUI: the kit reads TIFFs, never writes them, and builds the decoder halves of the libraries
 * libtiff's codecs call: zlib's inflate (kit/extern/zlib) and libjpeg-turbo's decompressor
 * (kit/extern/libjpeg-turbo/README.bui). tif_zip.c and tif_jpeg.c still name the encoders' entry
 * points; these stand in for them and fail the way those libraries fail on a bad call (never
 * reached: format_tiff.c opens files read-only). */
#include <stdio.h>

#include "zlib.h"

int ZEXPORT deflateInit_(z_streamp strm, int level, const char *version, int stream_size)
{
  (void)strm;
  (void)level;
  (void)version;
  (void)stream_size;
  return Z_STREAM_ERROR;
}
int ZEXPORT deflate(z_streamp strm, int flush)
{
  (void)strm;
  (void)flush;
  return Z_STREAM_ERROR;
}
int ZEXPORT deflateEnd(z_streamp strm)
{
  (void)strm;
  return Z_STREAM_ERROR;
}
int ZEXPORT deflateReset(z_streamp strm)
{
  (void)strm;
  return Z_STREAM_ERROR;
}
int ZEXPORT deflateParams(z_streamp strm, int level, int strategy)
{
  (void)strm;
  (void)level;
  (void)strategy;
  return Z_STREAM_ERROR;
}

#ifdef BUI_IMAGE_JPEG
#  include "jpeglib.h"
#  include "jerror.h"

#define FAIL(cinfo) ERREXIT1(cinfo, JERR_BAD_STATE, 0)

GLOBAL(void) jpeg_CreateCompress(j_compress_ptr cinfo, int version, size_t structsize)
{
  (void)version;
  (void)structsize;
  FAIL(cinfo);
}
GLOBAL(void) jpeg_set_defaults(j_compress_ptr cinfo)
{
  FAIL(cinfo);
}
GLOBAL(void) jpeg_set_colorspace(j_compress_ptr cinfo, J_COLOR_SPACE colorspace)
{
  (void)colorspace;
  FAIL(cinfo);
}
GLOBAL(void) jpeg_set_quality(j_compress_ptr cinfo, int quality, boolean force_baseline)
{
  (void)quality;
  (void)force_baseline;
  FAIL(cinfo);
}
GLOBAL(void) jpeg_suppress_tables(j_compress_ptr cinfo, boolean suppress)
{
  (void)suppress;
  FAIL(cinfo);
}
GLOBAL(void) jpeg_start_compress(j_compress_ptr cinfo, boolean write_all_tables)
{
  (void)write_all_tables;
  FAIL(cinfo);
}
GLOBAL(JDIMENSION) jpeg_write_scanlines(j_compress_ptr cinfo, JSAMPARRAY scanlines,
                                        JDIMENSION num_lines)
{
  (void)scanlines;
  (void)num_lines;
  FAIL(cinfo);
  return 0;
}
GLOBAL(JDIMENSION) jpeg_write_raw_data(j_compress_ptr cinfo, JSAMPIMAGE data,
                                       JDIMENSION num_lines)
{
  (void)data;
  (void)num_lines;
  FAIL(cinfo);
  return 0;
}
GLOBAL(void) jpeg_finish_compress(j_compress_ptr cinfo)
{
  FAIL(cinfo);
}
GLOBAL(void) jpeg_write_tables(j_compress_ptr cinfo)
{
  FAIL(cinfo);
}
#endif /* BUI_IMAGE_JPEG */
