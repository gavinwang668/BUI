/* BUI: OpenEXR's IlmThreadConfig.h (generated upstream from cmake/IlmThreadConfig.h.in).
 * OpenEXRCore uses it for its one-time initialization and its context locks; the kit's decode
 * threads read separate contexts at once. */
#ifndef INCLUDED_ILMTHREAD_CONFIG_H
#define INCLUDED_ILMTHREAD_CONFIG_H 1

#ifdef __wasi__
#  define ILMTHREAD_THREADING_ENABLED 0
#else
#  define ILMTHREAD_THREADING_ENABLED 1
#endif
#define ILMTHREAD_HAVE_POSIX_SEMAPHORES 0
#define ILMTHREAD_USE_TBB 0

#endif
