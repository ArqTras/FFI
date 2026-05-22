#!/usr/bin/env python3
"""Android cross: fix unbound configure OpenSSL probes (version, Windows libs)."""
import pathlib
import re
import sys

configure = pathlib.Path(sys.argv[1])
text = configure.read_text()
replacements = [
    (
        re.compile(
            r'as_fn_error\s+\$\?\s+"OpenSSL found in[^"]*version 0\.9\.7 or higher is required"',
        ),
        ': # arqma: skip openssl version probe',
    ),
    (
        re.compile(
            r'as_fn_error\s+\$\?\s+"OpenSSL 1\.0\.0 is needed for GOST support"',
        ),
        ': # arqma: skip gost openssl probe',
    ),
    (
        re.compile(r'^\tLIBS="\$LIBS -lcrypt32"$', re.M),
        '\t# arqma: no -lcrypt32',
    ),
    (
        re.compile(r'^\tLIBS="\$LIBS -lgdi32 -lws2_32 -lcrypt32', re.M),
        '\t# arqma: no win32 openssl libs: LIBS="$LIBS',
    ),
    (
        re.compile(r'^\tLIBSSL_LIBS="\$LIBSSL_LIBS -lgdi32 -lws2_32 -lcrypt32', re.M),
        '\t# arqma: LIBSSL_LIBS="$LIBSSL_LIBS',
    ),
]
total = 0
for pat, repl in replacements:
    text, n = pat.subn(repl, text)
    total += n
if total == 0:
    print('patch_configure_openssl: no match', file=sys.stderr)
    sys.exit(1)
configure.write_text(text)
print(f'patched {configure} ({total} replacement(s))')
