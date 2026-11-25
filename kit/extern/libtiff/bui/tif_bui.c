/* BUI: libtiff's platform layer (replaces tif_unix.c / tif_win32.c). The kit opens TIFFs from
 * memory only (TIFFClientOpenExt in format_tiff.c), so there is no TIFFOpen; memory comes from
 * Blender's guarded allocator (counted by MEM_get_memory_in_use() like the rest of the image
 * module), and libtiff's errors and warnings are not printed: a file that fails to decode
 * fails quietly, like every other format. */
#include "tiffiop.h"

#include <string.h>

#include "MEM_guardedalloc.h"

void *_TIFFmalloc(tmsize_t s)
{
  return s > 0 ? MEM_mallocN((size_t)s, "libtiff") : NULL;
}

void *_TIFFcalloc(tmsize_t nmemb, tmsize_t siz)
{
  return nmemb > 0 && siz > 0 ? MEM_calloc_arrayN((size_t)nmemb, (size_t)siz, "libtiff") : NULL;
}

void _TIFFfree(void *p)
{
  MEM_freeN(p);
}

void *_TIFFrealloc(void *p, tmsize_t s)
{
  if (s <= 0) {
    _TIFFfree(p);
    return NULL;
  }
  return p ? MEM_reallocN_id(p, (size_t)s, "libtiff") : MEM_mallocN((size_t)s, "libtiff");
}

void _TIFFmemset(void *p, int v, tmsize_t c)
{
  memset(p, v, (size_t)c);
}

void _TIFFmemcpy(void *d, const void *s, tmsize_t c)
{
  memcpy(d, s, (size_t)c);
}

int _TIFFmemcmp(const void *p1, const void *p2, tmsize_t c)
{
  return memcmp(p1, p2, (size_t)c);
}

TIFFErrorHandler _TIFFwarningHandler = NULL;
TIFFErrorHandler _TIFFerrorHandler = NULL;
