/**
 * Apply MinGW / Tauri native-wallet fixes to a cloned Arqma core tree (after clone, before CMake).
 * Idempotent — safe on every CI run.
 *
 * Usage: node patch-arqma-mingw-gui.js <arqma-upstream-root>
 */
/* eslint-disable no-template-curly-in-string -- CMake uses literal ${ARCH_ID}, not JS interpolation */
const fs = require("fs")
const path = require("path")

const up = process.argv[2]
if (!up) {
  console.error("usage: node patch-arqma-mingw-gui.js <arqma-upstream-root>")
  process.exit(1)
}

function patchRandomarqCMake () {
  const f = path.join(up, "external/randomarq/CMakeLists.txt")
  if (!fs.existsSync(f)) {
    return
  }
  let s = fs.readFileSync(f, "utf8")
  const normalizeAlready =
    "string(TOLOWER \"${ARCH_ID}\" ARCH_ID)"
  if (
    s.includes("Arqma-GUI-MM: normalize ARCH_ID for RandomX JIT") ||
    s.includes(normalizeAlready)
  ) {
    return
  }
  const insert =
    "\n# Arqma-GUI-MM: normalize ARCH_ID for RandomX JIT (Windows CMAKE may pass AMD64).\n" +
    "string(TOLOWER \"${ARCH_ID}\" ARCH_ID)\n\n"
  const reAfterArchEndifArm =
    /(endif\(\)\r?\n)(\s+if\(NOT ARM_ID\))/
  const reAfterProject =
    /(project\(RandomARQ\)\r?\n)/
  if (reAfterArchEndifArm.test(s)) {
    s = s.replace(reAfterArchEndifArm, (_, a, b) => a + insert + b)
  } else if (reAfterProject.test(s)) {
    s = s.replace(reAfterProject, (_, a) => a + insert)
  } else {
    console.warn("[patch-arqma-mingw-gui] skip randomarq CMakeLists.txt (pattern not found)")
    return
  }
  fs.writeFileSync(f, s)
  console.log("[patch-arqma-mingw-gui] patched external/randomarq/CMakeLists.txt")
}

function patchWalletCMake () {
  const f = path.join(up, "src/wallet/CMakeLists.txt")
  if (!fs.existsSync(f)) {
    return
  }
  let s = fs.readFileSync(f, "utf8")
  if (
    s.includes("Arqma-GUI-MM: wallet_merged MinGW daemonizer") ||
    s.includes("list(APPEND libs_to_merge daemonizer)")
  ) {
    return
  }
  // Match the desktop (non‑iOS) `libs_to_merge` list: it ends with `p2p` / `lmdb_lib)` then `else()`.
  // Older Arqma trees ended with `net)` only — support both shapes.
  const reDesktop =
    /(\s+p2p\s*\r?\n)(\s+lmdb_lib\)\s*\r?\n)(\s+else\(\))/
  const reLegacy =
    /(\s+checkpoints\s*\r?\n\s+version\s*\r?\n\s+net\)\s*\r?\n)(\s+else\(\))/
  let block
  if (reDesktop.test(s)) {
    block = (_, p2p, lmdbClose, elseLine) =>
      p2p +
      lmdbClose +
      "    # Arqma-GUI-MM: wallet_merged MinGW daemonizer\n" +
      "    # MinGW GUI link pulls `windows::check_admin` from daemonizer via cryptonote_core.\n" +
      "    if(MINGW)\n" +
      "      list(APPEND libs_to_merge daemonizer)\n" +
      "    endif()\n\n" +
      elseLine
    s = s.replace(reDesktop, block)
  } else if (reLegacy.test(s)) {
    block = (_, a, b) =>
      a +
      "    # Arqma-GUI-MM: wallet_merged MinGW daemonizer\n" +
      "    if(MINGW)\n" +
      "      list(APPEND libs_to_merge daemonizer)\n" +
      "    endif()\n\n" +
      b
    s = s.replace(reLegacy, block)
  } else {
    console.warn("[patch-arqma-mingw-gui] skip src/wallet/CMakeLists.txt (pattern not found)")
    return
  }
  fs.writeFileSync(f, s)
  console.log("[patch-arqma-mingw-gui] patched src/wallet/CMakeLists.txt")
}

function patchStackTrace () {
  const f = path.join(up, "src/common/stack_trace.cpp")
  if (!fs.existsSync(f)) {
    return
  }
  let s = fs.readFileSync(f, "utf8")
  if (s.includes("MinGW STATICLIB: Tauri cdylib is linked without GNU ld")) {
    return
  }
  const newDecl =
    "#ifdef STATICLIB\n" +
    "#if defined(__MINGW32__) || defined(__MINGW64__)\n" +
    "// MinGW STATICLIB: Tauri cdylib is linked without GNU ld `--wrap,__cxa_throw`; call libc++abi directly.\n" +
    "#define CXA_THROW __wrap___cxa_throw\n" +
    "extern \"C\"\n" +
    "__attribute__((noreturn))\n" +
    "void __cxa_throw(void *ex, CXA_THROW_INFO_T *info, void (*dest)(void*));\n" +
    "#else\n" +
    "#define CXA_THROW __wrap___cxa_throw\n" +
    "extern \"C\"\n" +
    "__attribute__((noreturn))\n" +
    "void __real___cxa_throw(void *ex, CXA_THROW_INFO_T *info, void (*dest)(void*));\n" +
    "#endif\n" +
    "#else // !STATICLIB"
  const declRe =
    /#ifdef STATICLIB\r?\n#define CXA_THROW __wrap___cxa_throw\r?\nextern "C"\r?\n__attribute__\(\(noreturn\)\)\r?\nvoid __real___cxa_throw\(void \*ex, CXA_THROW_INFO_T \*info, void \(\*dest\)\(void\*\)\);\r?\n#else \/\/ !STATICLIB/
  const tailRe =
    /#endif \/\/ !STATICLIB\r?\n[ \t]*__real___cxa_throw\(ex, info, dest\);\r?\n\}/
  const newTail =
    "#endif // !STATICLIB\n" +
    "#if defined(STATICLIB) && (defined(__MINGW32__) || defined(__MINGW64__))\n" +
    "  __cxa_throw(ex, info, dest);\n" +
    "#else\n" +
    "  __real___cxa_throw(ex, info, dest);\n" +
    "#endif\n" +
    "}"
  if (!declRe.test(s)) {
    console.warn("[patch-arqma-mingw-gui] skip stack_trace.cpp (STATICLIB decl block not found)")
    return
  }
  if (!tailRe.test(s)) {
    console.warn("[patch-arqma-mingw-gui] skip stack_trace.cpp (tail __real___cxa_throw call not found)")
    return
  }
  s = s.replace(declRe, newDecl)
  s = s.replace(tailRe, newTail)
  fs.writeFileSync(f, s)
  console.log("[patch-arqma-mingw-gui] patched src/common/stack_trace.cpp")
}

patchRandomarqCMake()
patchWalletCMake()
patchStackTrace()
