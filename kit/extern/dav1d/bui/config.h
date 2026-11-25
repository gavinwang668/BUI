/* BUI: dav1d's config.h (meson generates it upstream: meson.build, "Generate config.h"), for
 * the C-only build kit/CMakeLists.txt makes: no assembly (HAVE_ASM 0, so no ARCH_* code paths),
 * 8- and 16-bit pixels (the *_tmpl.c files compiled once per BITDEPTH), no logging. */
#ifndef BUI_DAV1D_CONFIG_H
#define BUI_DAV1D_CONFIG_H

#define CONFIG_8BPC 1
#define CONFIG_16BPC 1
#define CONFIG_LOG 0
#define CONFIG_ASM_DEBUG 0
#define HAVE_ASM 0
#define TRIM_DSP_FUNCTIONS 0

#define ARCH_AARCH64 0
#define ARCH_ARM 0
#define ARCH_LOONGARCH 0
#define ARCH_LOONGARCH32 0
#define ARCH_LOONGARCH64 0
#define ARCH_PPC64LE 0
#define ARCH_RISCV 0
#define ARCH_RV32 0
#define ARCH_RV64 0
#define ARCH_X86 0
#define ARCH_X86_32 0
#define ARCH_X86_64 0

#if defined(__BYTE_ORDER__) && __BYTE_ORDER__ == __ORDER_BIG_ENDIAN__
#  define ENDIANNESS_BIG 1
#else
#  define ENDIANNESS_BIG 0
#endif

#define HAVE_C11_GENERIC 1
#define HAVE_CLOCK_GETTIME 0 /* the tools' */
#define HAVE_SIGACTION 0
#define HAVE_DLSYM 0 /* __pthread_get_minstack: dav1d then sizes thread stacks itself */
#define HAVE_GETAUXVAL 0
#define HAVE_ELF_AUX_INFO 0
#define HAVE_PTHREAD_NP_H 0
#define HAVE_PTHREAD_GETAFFINITY_NP 0
#define HAVE_PTHREAD_SETAFFINITY_NP 0
#define HAVE_PTHREAD_SETNAME_NP 0
#define HAVE_PTHREAD_SET_NAME_NP 0

#ifdef _WIN32
#  define HAVE_UNISTD_H 0
#  define HAVE_IO_H 1
#  define HAVE_SYS_TYPES_H 1
#  define HAVE_POSIX_MEMALIGN 0
#  define HAVE_MEMALIGN 0
#  define HAVE_ALIGNED_ALLOC 0
#  ifndef _WIN32_WINNT
#    define _WIN32_WINNT 0x0601
#  endif
#  ifndef UNICODE
#    define UNICODE 1
#  endif
#  ifndef _UNICODE
#    define _UNICODE 1
#  endif
#  define __USE_MINGW_ANSI_STDIO 1
#  define _CRT_DECLARE_NONSTDC_NAMES 1
#else
#  define HAVE_UNISTD_H 1
#  define HAVE_IO_H 0
#  define HAVE_SYS_TYPES_H 1
#  define HAVE_POSIX_MEMALIGN 1
#  define HAVE_MEMALIGN 0
#  define HAVE_ALIGNED_ALLOC 0
#endif

#endif /* BUI_DAV1D_CONFIG_H */
