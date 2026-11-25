/* BUI: libjxl's jxl_export.h (CMake's generate_export_header writes it upstream, lib/jxl.cmake).
 * The decoder is compiled into the kit (JXL_STATIC_DEFINE): nothing is exported. */
#ifndef JXL_EXPORT_H
#define JXL_EXPORT_H

#define JXL_EXPORT
#define JXL_NO_EXPORT
#ifdef _MSC_VER
#  define JXL_DEPRECATED __declspec(deprecated)
#else
#  define JXL_DEPRECATED __attribute__((__deprecated__))
#endif
#define JXL_DEPRECATED_EXPORT JXL_EXPORT JXL_DEPRECATED
#define JXL_DEPRECATED_NO_EXPORT JXL_NO_EXPORT JXL_DEPRECATED

#endif /* JXL_EXPORT_H */
