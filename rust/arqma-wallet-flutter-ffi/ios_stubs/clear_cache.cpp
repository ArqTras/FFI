#include <stddef.h>

#if defined(__APPLE__)
#include <libkern/OSCacheControl.h>
#endif

extern "C" void __clear_cache(void *begin, void *end) {
  if (!begin || !end || end <= begin) {
    return;
  }
#if defined(__APPLE__)
  sys_icache_invalidate(begin, (size_t)((char *)end - (char *)begin));
#else
  __builtin___clear_cache((char *)begin, (char *)end);
#endif
}
