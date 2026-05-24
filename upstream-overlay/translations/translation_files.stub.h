#ifndef TRANSLATION_FILES_H
#define TRANSLATION_FILES_H

#include <string>

// Stub for mobile/iOS FFI builds (no Qt lrelease / embedded .qm in libwallet_merged).
static const struct embedded_file {
  const std::string *name;
  const std::string *data;
} embedded_files[] = {
  {NULL, NULL}
};

static bool find_embedded_file(const std::string &name, std::string &data) {
  (void)name;
  (void)data;
  return false;
}

#endif /* TRANSLATION_FILES_H */
