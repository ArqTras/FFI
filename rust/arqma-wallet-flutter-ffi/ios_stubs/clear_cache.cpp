#include <libkern/OSCacheControl.h>
#include <stddef.h>

extern "C" void __clear_cache(void *begin, void *end) {
  if (begin && end && end > begin) {
    sys_icache_invalidate(begin, (size_t)((char *)end - (char *)begin));
  }
}
