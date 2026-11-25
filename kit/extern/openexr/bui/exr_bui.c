/* BUI: the compressions OpenEXRCore takes from other libraries, which the kit leaves out: zstd
 * (internal_zstd*.c, the vendored zstd) and HTJ2K (internal_ht*.cpp, OpenJPH, C++). Parts
 * compressed with them fail to decode, as with a library built without them. */
#include "internal_compress.h"
#include "internal_decompress.h"

exr_result_t internal_exr_apply_ht(exr_encode_pipeline_t *encode)
{
  (void)encode;
  return EXR_ERR_FEATURE_NOT_IMPLEMENTED;
}

exr_result_t internal_exr_undo_ht(exr_decode_pipeline_t *decode, const void *compressed_data,
                                  uint64_t comp_buf_size, void *uncompressed_data,
                                  uint64_t uncompressed_size)
{
  (void)decode;
  (void)compressed_data;
  (void)comp_buf_size;
  (void)uncompressed_data;
  (void)uncompressed_size;
  return EXR_ERR_FEATURE_NOT_IMPLEMENTED;
}

exr_result_t internal_exr_apply_zstd(exr_encode_pipeline_t *encode)
{
  (void)encode;
  return EXR_ERR_FEATURE_NOT_IMPLEMENTED;
}

exr_result_t internal_exr_undo_zstd(exr_decode_pipeline_t *decode, const void *compressed_data,
                                    uint64_t comp_buf_size, void *uncompressed_data,
                                    uint64_t uncompressed_size)
{
  (void)decode;
  (void)compressed_data;
  (void)comp_buf_size;
  (void)uncompressed_data;
  (void)uncompressed_size;
  return EXR_ERR_FEATURE_NOT_IMPLEMENTED;
}
