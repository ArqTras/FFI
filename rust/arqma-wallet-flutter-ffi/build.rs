//! Final-link flags for the `cdylib` wallet stack (same MinGW / Linux needs as `rust/tauri-app/src-tauri/build.rs`).
//! **Upstream archives:** `libepee.a`, `libeasylogging.a`, `libcryptonote_format_utils_basic.a`, `liblmdb.a` need
//! `-Wl,--whole-archive` or link fails with undefined symbols.
//!
//! **Default:** native deps linked as **dynamic** where `arqma-wallet2-api` emits `rustc-link-lib=dylib` (Linux/macOS),
//! or MSYS2-style `-l…` on **windows-gnu** (this crate always appends those flags on MinGW).
//!
//! **Experimental:** `ARQMA_WALLET_FFI_STATIC_HYBRID=1` — fold Boost/OpenSSL/libsodium/hidapi/readline (+ **zmq** / **unbound**
//! when possible) into the FFI `cdylib` for portable bundles.
//!
//! **Linux/macOS:** if **`contrib/depends/<host>/lib`** exists under **`ARQMA_WALLET2_UPSTREAM_DIR`** (or **`ARQMA_WALLET_FFI_DEPENDS_LIB_DIR`**),
//! `build.rs` prepends that path so `-l…` resolves to **PIC** static archives from `make depends` — then **zmq** and **unbound**
//! are linked **statically** too (full hybrid). Otherwise **zmq** / **unbound** stay **dynamic** (distro `.a` is usually not PIC).
//! **ICU:** when **`libicuuc.a`**, **`libicui18n.a`**, and **`libicudata.a`** are present in that same `lib/` (from
//! `build/ci/build-icu-static-into-depends.sh`), they are folded **statically**; otherwise ICU stays **dynamic**.
//! **`libstdc++`** (Linux) typically stays **dynamic** when using vendored Boost.
//! On Linux/macOS set this env when building `arqma-wallet-flutter-ffi` so `arqma-wallet2-api` skips duplicate `dylib` lines
//! (see `arqma-wallet2-api/build.rs`).

use std::path::{Path, PathBuf};

fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    println!("cargo:rerun-if-env-changed=ARQMA_WALLET_FFI_STATIC_HYBRID");
    println!("cargo:rerun-if-env-changed=ARQMA_WALLET_FFI_DEPENDS_LIB_DIR");

    if target_os == "linux" || target_os == "android" {
        println!("cargo:rustc-link-arg=-Wl,-z,muldefs");
    }

    if target_os == "ios" || target_os == "android" {
        compile_clear_cache_stub();
    }

    if target_os == "windows" && target_env == "gnu" {
        mingw_wallet2_native_libs_cdylib_args();
    } else if target_os == "linux" && static_hybrid_enabled() {
        println!(
            "cargo:warning=arqma-wallet-flutter-ffi: ARQMA_WALLET_FFI_STATIC_HYBRID=1 (Linux static-hybrid)"
        );
        linux_wallet_ffi_static_hybrid_cdylib_args();
    } else if target_os == "macos" && static_hybrid_enabled() {
        println!(
            "cargo:warning=arqma-wallet-flutter-ffi: ARQMA_WALLET_FFI_STATIC_HYBRID=1 (macOS static-hybrid)"
        );
        macos_wallet_ffi_static_hybrid_cdylib_args();
    } else if target_os == "ios" && static_hybrid_enabled() {
        println!(
            "cargo:warning=arqma-wallet-flutter-ffi: ARQMA_WALLET_FFI_STATIC_HYBRID=1 (iOS static-hybrid)"
        );
        ios_wallet_ffi_static_hybrid_cdylib_args();
    } else if target_os == "android" && static_hybrid_enabled() {
        println!(
            "cargo:warning=arqma-wallet-flutter-ffi: ARQMA_WALLET_FFI_STATIC_HYBRID=1 (Android static-hybrid)"
        );
        android_wallet_ffi_static_hybrid_cdylib_args();
    }
}

fn android_wallet_ffi_static_hybrid_cdylib_args() {
    let emit = |flag: &str| println!("cargo:rustc-cdylib-link-arg={flag}");

    let upstream = arqma_upstream_root();
    let vendor = depends_vendor_lib_dir(&upstream);
    if let Some(ref libdir) = vendor {
        println!(
            "cargo:rustc-link-search=native={}",
            libdir.display().to_string().replace('\\', "/")
        );
        println!(
            "cargo:warning=arqma-wallet-flutter-ffi: Android static-hybrid uses contrib/depends ({})",
            libdir.display()
        );
    }

    emit("-Wl,--no-as-needed");
    emit("-Wl,-Bdynamic");
    emit("-lc++_shared");

    emit("-Wl,-Bstatic");
    emit("-static-libgcc");
    emit("-Wl,--start-group");
    let android_libs: &[&str] = &[
        "boost_program_options",
        "boost_thread",
        "boost_date_time",
        "unbound",
        "boost_filesystem",
        "boost_atomic",
        "boost_chrono",
        "ssl",
        "crypto",
        "boost_serialization",
        "boost_regex",
        "zmq",
        "sodium",
    ];
    match &vendor {
        Some(libdir) => {
            for stem in android_libs {
                emit_depends_vendor_lib(&emit, libdir, stem);
            }
        }
        None => {
            for stem in android_libs {
                emit(&format!("-l{stem}"));
            }
        }
    }
    emit("-Wl,--end-group");

    emit("-Wl,-Bdynamic");
    emit("-ldl");
    emit("-llog");
    emit("-lm");
}

fn compile_clear_cache_stub() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let mut build = cc::Build::new();
    build.cpp(true).file("ios_stubs/clear_cache.cpp");
    if target_os == "ios" {
        let sdk = std::process::Command::new("xcrun")
            .args(["--sdk", "iphoneos", "--show-sdk-path"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();
        build.flag("-arch").flag("arm64");
        build.flag("-miphoneos-version-min=13.0");
        if !sdk.is_empty() {
            build.flag(format!("-isysroot{sdk}"));
        }
    }
    build.compile("arqma_clear_cache_stub");
}

fn static_hybrid_enabled() -> bool {
    std::env::var("ARQMA_WALLET_FFI_STATIC_HYBRID")
        .map(|v| matches!(v.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

fn mingw_wallet2_native_libs_cdylib_args() {
    let hybrid = static_hybrid_enabled();
    let emit = |flag: &str| {
        println!("cargo:rustc-cdylib-link-arg={flag}");
    };

    emit("-Wl,--no-as-needed");
    // GitHub `wallet_merged` already folds easylogging/epee/…; re-linking the same `.a` files then
    // causes GNU ld "multiple definition". Local/offline merges may omit them — keep aux archives.
    // Prefer resolving duplicates over missing symbols (identical object code in practice).
    emit("-Wl,--allow-multiple-definition");
    emit_upstream_aux_archives(&emit);

    if hybrid {
        emit("-Wl,-Bstatic");
        emit("-static-libgcc");
        // Do not use `-static-libstdc++` here: with Rust's link order it often fails to pull the full
        // archive before `-Wl,-Bdynamic`/ICU; link then misses libstdc++ symbols from `libepee.a`/Boost.
        // Boost/OpenSSL/… stay static via `-Bstatic` + `-l…`; libstdc++ remains dynamic (`-lstdc++` below).
    }

    emit("-Wl,--start-group");
    for lib in mingw_wallet_dep_libs() {
        emit(&format!("-l{lib}"));
    }
    emit("-Wl,--end-group");

    if let Some(rx) = upstream_librandomx_a_path() {
        emit(&path_for_ld(&rx));
    }

    if hybrid {
        emit("-Wl,-Bdynamic");
    }

    for lib in ["icuuc", "icuin", "icudt", "iconv"] {
        emit(&format!("-l{lib}"));
    }

    for lib in mingw_windows_system_libs() {
        emit(&format!("-l{lib}"));
    }
    emit("-lm");
    emit("-lmingwex");
    // `libmingwex` (e.g. gettimeofday) may need kernel32 imports resolved after the CRT block.
    emit("-lkernel32");
    emit("-lunwind");
    emit("-lstdc++");

    emit("-lmingw32");
    emit("-lmsvcrt");
    emit("-Wl,--no-gc-sections");
}

fn linux_wallet_ffi_static_hybrid_cdylib_args() {
    let emit = |flag: &str| println!("cargo:rustc-cdylib-link-arg={flag}");

    let upstream = arqma_upstream_root();
    let vendor = depends_vendor_lib_dir(&upstream);
    if let Some(ref libdir) = vendor {
        println!(
            "cargo:rustc-link-search=native={}",
            libdir.display().to_string().replace('\\', "/")
        );
        println!(
            "cargo:warning=arqma-wallet-flutter-ffi: static-hybrid prefers contrib/depends ({})",
            libdir.display()
        );
    }

    emit("-Wl,--no-as-needed");
    emit_upstream_aux_archives(&emit);

    if vendor.is_some() {
        // Pull shared libstdc++ before -Bstatic/--start-group so GNU ld does not satisfy C++ symbols from
        // libboost*.a by linking libstdc++.a (non-PIC) into the cdylib.
        emit("-Wl,-Bdynamic");
        emit("-lstdc++");
    }

    emit("-Wl,-Bstatic");
    emit("-static-libgcc");

    emit("-Wl,--start-group");
    let linux_libs: &[&str] = if vendor.is_some() {
        linux_hybrid_full_static_dep_libs()
    } else {
        linux_hybrid_static_dep_libs()
    };
    match &vendor {
        Some(libdir) => {
            for stem in linux_libs {
                emit_depends_vendor_lib(&emit, libdir, stem);
            }
            emit_depends_icu_static_if_present(&emit, libdir);
        }
        None => {
            for stem in linux_libs {
                emit(&format!("-l{stem}"));
            }
        }
    }
    emit("-Wl,--end-group");

    if let Some(rx) = upstream_librandomx_a_path() {
        emit(&path_for_ld(&rx));
    }

    emit("-Wl,-Bdynamic");
    if vendor.is_none() {
        println!("cargo:rustc-link-lib=dylib=zmq");
        println!("cargo:rustc-link-lib=dylib=unbound");
    }

    if vendor
        .as_ref()
        .and_then(|d| depends_vendor_icu_static_triple(d))
        .is_none()
    {
        for lib in ["icuuc", "icui18n", "icudata"] {
            emit(&format!("-l{lib}"));
        }
    }

    emit("-lz");
    emit("-ldl");
    emit("-lpthread");
    emit("-lm");
    emit("-lresolv");
    emit("-ltinfo");
    if vendor.is_none() {
        emit("-lstdc++");
    }
    // Remaining dynamic deps (e.g. zlib) may still be resolved from global paths; `$ORIGIN` picks up bundled `.so` copies.
    emit("-Wl,-z,origin");
    emit("-Wl,-rpath,$ORIGIN");
}

fn macos_wallet_ffi_static_hybrid_cdylib_args() {
    let emit = |flag: &str| println!("cargo:rustc-cdylib-link-arg={flag}");

    let upstream = arqma_upstream_root();
    let vendor = depends_vendor_lib_dir(&upstream);
    if let Some(ref libdir) = vendor {
        println!(
            "cargo:rustc-link-search=native={}",
            libdir.display().to_string().replace('\\', "/")
        );
        println!(
            "cargo:warning=arqma-wallet-flutter-ffi: static-hybrid prefers contrib/depends ({})",
            libdir.display()
        );
    }

    emit("-Wl,-search_paths_first,-headerpad_max_install_names");
    // Avoid GNU ld-only flags (`--no-as-needed`, `--start-group`) — Apple ld / Rust's linker driver mishandles them.
    emit_upstream_aux_archives(&emit);

    let mac_libs: &[&str] = if vendor.is_some() {
        macos_hybrid_full_static_dep_libs()
    } else {
        macos_hybrid_static_dep_libs()
    };
    match &vendor {
        Some(libdir) => {
            for stem in mac_libs {
                emit_depends_vendor_lib(&emit, libdir, stem);
            }
            emit_depends_icu_static_if_present(&emit, libdir);
        }
        None => {
            for stem in mac_libs {
                emit(&format!("-l{stem}"));
            }
        }
    }

    // `arqma-wallet2-api` on macOS links `wallet_merged` + `lmdb` only; merged already folds epee /
    // easylogging / randomx / cryptonote. Do not `force_load` standalone `librandomx.a` here.

    if vendor.is_none() {
        println!("cargo:rustc-link-lib=dylib=zmq");
        println!("cargo:rustc-link-lib=dylib=unbound");
    }

    if vendor
        .as_ref()
        .and_then(|d| depends_vendor_icu_static_triple(d))
        .is_none()
    {
        for lib in ["icuuc", "icui18n", "icudata"] {
            emit(&format!("-l{lib}"));
        }
    }
    emit("-lz");
    println!("cargo:rustc-link-lib=framework=AppKit");
    println!("cargo:rustc-link-lib=framework=IOKit");
    println!("cargo:rustc-link-lib=framework=CoreFoundation");
}

fn ios_wallet_ffi_static_hybrid_cdylib_args() {
    let emit = |flag: &str| println!("cargo:rustc-cdylib-link-arg={flag}");

    let upstream = arqma_upstream_root();
    let vendor = depends_vendor_lib_dir(&upstream);
    if let Some(ref libdir) = vendor {
        println!(
            "cargo:rustc-link-search=native={}",
            libdir.display().to_string().replace('\\', "/")
        );
        println!(
            "cargo:warning=arqma-wallet-flutter-ffi: iOS static-hybrid uses contrib/depends ({})",
            libdir.display()
        );
    }

    emit("-miphoneos-version-min=13.0");
    emit_upstream_aux_archives(&emit);
    emit_ios_wallet_aux_archives(&emit);

    // OpenSSL: libssl depends on libcrypto — link crypto before ssl when force-loading.
    let ios_libs: &[&str] = &[
        "boost_program_options",
        "boost_thread",
        "boost_date_time",
        "unbound",
        "boost_filesystem",
        "boost_atomic",
        "boost_chrono",
        "crypto",
        "ssl",
        "boost_serialization",
        "boost_regex",
        "zmq",
        "sodium",
    ];
    match &vendor {
        Some(libdir) => {
            for stem in ios_libs {
                emit_depends_vendor_lib_force_load(&emit, libdir, stem);
            }
            emit_depends_icu_static_if_present(&emit, libdir);
        }
        None => {
            for stem in ios_libs {
                emit(&format!("-l{stem}"));
            }
        }
    }

    if vendor
        .as_ref()
        .and_then(|d| depends_vendor_icu_static_triple(d))
        .is_none()
    {
        println!(
            "cargo:warning=arqma-wallet-flutter-ffi: ICU static libs not in contrib/depends for iOS — run build/ci/build-icu-static-into-depends.sh if boost_locale symbols are missing"
        );
    }
    emit("-lz");
    for fw in [
        "Security",
        "Foundation",
        "SystemConfiguration",
        "CFNetwork",
        "UIKit",
    ] {
        println!("cargo:rustc-link-lib=framework={fw}");
    }
}

fn mingw_wallet_dep_libs() -> &'static [&'static str] {
    &[
        "boost_atomic-mt",
        "boost_container-mt",
        "boost_filesystem-mt",
        "boost_thread-mt",
        "boost_chrono-mt",
        "boost_date_time-mt",
        "boost_serialization-mt",
        "boost_program_options-mt",
        "boost_locale-mt",
        "ssl",
        "crypto",
        "zmq",
        "sodium",
        "hidapi",
        "unbound",
        // `readline_buffer.cpp` in folded epee / cryptonote archives (MSYS2 GNU readline).
        "readline",
        "history",
        "ncurses",
    ]
}

/// `contrib/depends` PIC static libs — full set for portable `cdylib` (includes zmq + unbound).
fn linux_hybrid_full_static_dep_libs() -> &'static [&'static str] {
    &[
        "hidapi-libusb",
        "usb-1.0",
        "boost_program_options",
        "boost_thread",
        "boost_container",
        "boost_date_time",
        "unbound",
        "boost_filesystem",
        "boost_atomic",
        "boost_chrono",
        "ssl",
        "crypto",
        "readline",
        "boost_serialization",
        "boost_regex",
        "boost_locale",
        "zmq",
        "sodium",
    ]
}

/// Host static libs only (no zmq/unbound) when `contrib/depends` is absent — see module doc.
fn linux_hybrid_static_dep_libs() -> &'static [&'static str] {
    &[
        "hidapi-libusb",
        "usb-1.0",
        "boost_program_options",
        "boost_thread",
        "boost_container",
        "boost_date_time",
        "boost_filesystem",
        "boost_atomic",
        "boost_chrono",
        "ssl",
        "crypto",
        "readline",
        "boost_serialization",
        "boost_regex",
        "boost_locale",
        "sodium",
    ]
}

fn macos_hybrid_full_static_dep_libs() -> &'static [&'static str] {
    &[
        "hidapi",
        "boost_program_options",
        "boost_thread",
        "boost_container",
        "boost_date_time",
        "unbound",
        "boost_filesystem",
        "boost_atomic",
        "boost_chrono",
        "ssl",
        "crypto",
        "readline",
        "boost_serialization",
        "boost_regex",
        "boost_locale",
        "zmq",
        "sodium",
    ]
}

/// iOS `contrib/depends`: no HID/readline; boost_locale/container omitted (no ICU in depends yet).
fn ios_hybrid_full_static_dep_libs() -> &'static [&'static str] {
    &[
        "boost_program_options",
        "boost_thread",
        "boost_date_time",
        "unbound",
        "boost_filesystem",
        "boost_atomic",
        "boost_chrono",
        "ssl",
        "crypto",
        "boost_serialization",
        "boost_regex",
        "zmq",
        "sodium",
    ]
}

/// Host static libs only when `contrib/depends` is absent — see module doc.
fn macos_hybrid_static_dep_libs() -> &'static [&'static str] {
    &[
        "hidapi",
        "boost_program_options",
        "boost_thread",
        "boost_container",
        "boost_date_time",
        "boost_filesystem",
        "boost_atomic",
        "boost_chrono",
        "ssl",
        "crypto",
        "readline",
        "boost_serialization",
        "boost_regex",
        "boost_locale",
        "sodium",
    ]
}

fn mingw_windows_system_libs() -> &'static [&'static str] {
    &[
        "ws2_32",
        "mswsock",
        "iphlpapi",
        "crypt32",
        "advapi32",
        "shell32",
        "userenv",
        "user32",
        "kernel32",
    ]
}

fn path_for_ld(p: &Path) -> String {
    p.display().to_string().replace('\\', "/")
}

/// Candidate static `.a` names under `contrib/depends/.../lib` for a `-l` stem (order matters).
fn depends_vendor_archive_paths(libdir: &Path, stem: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    match stem {
        "ssl" => out.push(libdir.join("libssl.a")),
        "crypto" => out.push(libdir.join("libcrypto.a")),
        s if s.starts_with("boost_") => {
            out.push(libdir.join(format!("lib{s}.a")));
            out.push(libdir.join(format!("lib{s}-mt.a")));
        }
        "hidapi-libusb" => {
            out.push(libdir.join("libhidapi-libusb.a"));
            out.push(libdir.join("libhidapi-hidraw.a"));
            out.push(libdir.join("libhidapi.a"));
        }
        "hidapi" => {
            out.push(libdir.join("libhidapi.a"));
            out.push(libdir.join("libhidapi-libusb.a"));
            out.push(libdir.join("libhidapi-hidraw.a"));
        }
        _ => out.push(libdir.join(format!("lib{stem}.a"))),
    }
    out
}

/// Static ICU archives from `build/ci/build-icu-static-into-depends.sh` (same `contrib/depends/.../lib` tree).
fn depends_vendor_icu_static_triple(libdir: &Path) -> Option<(PathBuf, PathBuf, PathBuf)> {
    let data = libdir.join("libicudata.a");
    let i18n = libdir.join("libicui18n.a");
    let uc = libdir.join("libicuuc.a");
    if data.is_file() && i18n.is_file() && uc.is_file() {
        Some((data, i18n, uc))
    } else {
        None
    }
}

fn emit_depends_icu_static_if_present(emit: &dyn Fn(&str), libdir: &Path) {
    if let Some((data, i18n, uc)) = depends_vendor_icu_static_triple(libdir) {
        println!(
            "cargo:warning=arqma-wallet-flutter-ffi: linking static ICU from {}",
            libdir.display()
        );
        emit(&path_for_ld(&data));
        emit(&path_for_ld(&i18n));
        emit(&path_for_ld(&uc));
    }
}

/// `contrib/depends` Boost packages often use non-canonical names (e.g. `libboost_thread-mt-x64.a`).
/// Pick the first matching `lib{stem}*.a` so we never fall back to `-l` (GCC would resolve host `libboost_*.a`).
fn depends_vendor_archive_fuzzy_boost(libdir: &Path, stem: &str) -> Option<PathBuf> {
    if !stem.starts_with("boost_") {
        return None;
    }
    let prefix = format!("lib{stem}");
    let mut hits: Vec<PathBuf> = std::fs::read_dir(libdir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension().and_then(|x| x.to_str()) == Some("a")
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| {
                        if !n.starts_with(&prefix) {
                            return false;
                        }
                        let tail = &n[prefix.len()..];
                        tail.is_empty() || tail.starts_with('-') || tail.starts_with('.')
                    })
                    .unwrap_or(false)
        })
        .collect();
    hits.sort();
    hits.into_iter().next()
}

/// iOS: `-dead_strip` on the cdylib can drop needed objects from vendored `.a` unless force-loaded.
fn emit_depends_vendor_lib_force_load(emit: &dyn Fn(&str), libdir: &Path, stem: &str) {
    for p in depends_vendor_archive_paths(libdir, stem) {
        if p.is_file() {
            emit("-Wl,-force_load");
            emit(&path_for_ld(&p));
            return;
        }
    }
    if let Some(p) = depends_vendor_archive_fuzzy_boost(libdir, stem) {
        emit("-Wl,-force_load");
        emit(&path_for_ld(&p));
        return;
    }
    println!(
        "cargo:warning=arqma-wallet-flutter-ffi: contrib/depends has no static archive for `{stem}` under {}",
        libdir.display()
    );
    emit(&format!("-l{stem}"));
}

/// Emit an absolute path to a vendored `.a` so the linker does not fall back to GCC/Homebrew `-l` search paths.
fn emit_depends_vendor_lib(emit: &dyn Fn(&str), libdir: &Path, stem: &str) {
    for p in depends_vendor_archive_paths(libdir, stem) {
        if p.is_file() {
            emit(&path_for_ld(&p));
            return;
        }
    }
    if let Some(p) = depends_vendor_archive_fuzzy_boost(libdir, stem) {
        emit(&path_for_ld(&p));
        return;
    }
    println!(
        "cargo:warning=arqma-wallet-flutter-ffi: contrib/depends has no static archive for `{stem}` under {}",
        libdir.display()
    );
    emit(&format!("-l{stem}"));
}

fn arqma_upstream_root() -> PathBuf {
    std::env::var("ARQMA_WALLET2_UPSTREAM_DIR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
                .join("../arqma-rpc-upstream")
        })
}

/// Matches `contrib/depends` hosts used in `build/ci/build-arqma-wallet-ffi-deps.sh`.
fn depends_host_triple() -> Option<&'static str> {
    let os = std::env::var("CARGO_CFG_TARGET_OS").ok()?;
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").ok()?;
    match (os.as_str(), arch.as_str()) {
        ("linux", "x86_64") => Some("x86_64-unknown-linux-gnu"),
        ("macos", "aarch64") => Some("aarch64-apple-darwin"),
        ("macos", "x86_64") => Some("x86_64-apple-darwin"),
        ("ios", "aarch64") => Some("aarch64-apple-ios"),
        ("android", "aarch64") => Some("aarch64-linux-android"),
        ("android", "x86_64") => Some("x86_64-linux-android"),
        ("android", "arm") => Some("armv7-linux-androideabi"),
        _ => None,
    }
}

/// Vendored static/PIC libs from `make -C contrib/depends` (full hybrid when present).
fn depends_vendor_lib_dir(upstream: &Path) -> Option<PathBuf> {
    if let Ok(p) = std::env::var("ARQMA_WALLET_FFI_DEPENDS_LIB_DIR") {
        let pb = PathBuf::from(p.trim());
        if pb.is_dir() {
            return Some(pb);
        }
    }
    let host = depends_host_triple()?;
    let lib = upstream.join("contrib/depends").join(host).join("lib");
    if !lib.is_dir() {
        return None;
    }
    if lib.join("libssl.a").is_file() || lib.join("libzmq.a").is_file() {
        return Some(lib);
    }
    None
}

fn emit_ios_wallet_aux_archives(emit: &dyn Fn(&str)) {
    let upstream = arqma_upstream_root();
    for sub in ["build-ios-depends-device", "build-ios-device"] {
        let root = upstream.join(sub);
        let merged = root.join("src/wallet/libwallet_merged.a");
        // `arqma-wallet2-api` links `wallet_merged` + `lmdb` (+whole-archive). Folded `wallet_merged`
        // still needs standalone `librandomx.a` and often `libeasylogging.a` for logging symbols.
        // Never force_load epee or lmdb here (duplicate symbols with merged + wallet2-api).
        let aux = [
            root.join("contrib/epee/src/libepee.a"),
            root.join("external/randomarq/librandomx.a"),
            root.join("external/easylogging++/libeasylogging.a"),
        ];
        let mut linked = false;
        for path in aux {
            if path.is_file() {
                emit("-Wl,-force_load");
                emit(&path_for_ld(&path));
                linked = true;
            }
        }
        if merged.is_file() && linked {
            return;
        }
        if merged.is_file() {
            return;
        }
    }
    println!(
        "cargo:warning=arqma-wallet-flutter-ffi: iOS wallet_merged not found under build-ios-depends-device"
    );
}

fn emit_upstream_aux_archives(emit: &dyn Fn(&str)) {
    let upstream = arqma_upstream_root();
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let macos = target_os == "macos";
    // iOS / Android: epee is folded into `wallet_merged` by fold-wallet-merged-archive.sh.
    if target_os == "ios" || target_os == "android" {
        return;
    }
    // MinGW: `wallet_merged` folds epee/easylogging/daemonizer/randomx (CMake POST_BUILD). Re-whole-archiving
    // those `.a` files duplicates static init → LoadLibrary Win32 1114 (FFI 1.0.5). The C LMDB engine
    // (`mdb.c`) stays in `liblmdb.a` only — `obj_lmdb_lib` in merged is the C++ wrapper, not `mdb_*` defs.
    if target_os == "windows" && target_env == "gnu" {
        for sub in [
            "build/ci-depends-release",
            "build-mingw",
            "build/ci-native-release",
            "build",
        ] {
            let lmdb = upstream.join(sub).join("src/lmdb/liblmdb/liblmdb.a");
            if lmdb.is_file() {
                println!(
                    "cargo:warning=arqma-wallet-flutter-ffi: windows-gnu link LMDB only ({})",
                    lmdb.display()
                );
                emit("-Wl,--whole-archive");
                emit(&path_for_ld(&lmdb));
                emit("-Wl,--no-whole-archive");
                return;
            }
        }
        println!(
            "cargo:warning=arqma-wallet-flutter-ffi: windows-gnu LMDB archive not found; wallet link may fail"
        );
        return;
    }
    // CI macOS/Linux use `build/ci-native-release` (see build/ci/build-arqma-*.sh); MinGW uses `build-mingw`.
    for sub in [
        "build/ci-depends-release",
        "build-mingw",
        "build/ci-native-release",
        "build",
    ] {
        let root = upstream.join(sub);
        let epee = root.join("contrib/epee/src/libepee.a");
        let elog = root.join("external/easylogging++/libeasylogging.a");
        let cn = root.join("src/cryptonote_basic/libcryptonote_format_utils_basic.a");
        let lmdb = root.join("src/lmdb/liblmdb/liblmdb.a");
        if epee.is_file() && elog.is_file() && cn.is_file() && lmdb.is_file() {
            if macos {
                // `wallet_merged` on macOS already contains these objects; `arqma-wallet2-api` links
                // `wallet_merged` + `lmdb` via `#[link]`. Re-linking the split `.a` files causes hundreds
                // of duplicate symbols (Apple ld has no `-z muldefs`).
                return;
            } else {
                emit("-Wl,--whole-archive");
                emit(&path_for_ld(&epee));
                emit(&path_for_ld(&elog));
                emit(&path_for_ld(&cn));
                emit(&path_for_ld(&lmdb));
                emit("-Wl,--no-whole-archive");
            }
            return;
        }
    }
    println!(
        "cargo:warning=arqma-wallet-flutter-ffi: upstream aux archives (epee/easylogging/cryptonote/lmdb) not found under build-mingw, build/ci-native-release, or build - wallet link may fail"
    );
}

fn upstream_librandomx_a_path() -> Option<PathBuf> {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "android" || target_os == "ios" {
        return None;
    }
    let upstream = arqma_upstream_root();
    for sub in [
        "build-android-depends-aarch64-linux-android",
        "build-android-depends-x86_64-linux-android",
        "build/ci-depends-release",
        "build-mingw",
        "build/ci-native-release",
        "build",
    ] {
        let lib = upstream.join(sub).join("external/randomarq/librandomx.a");
        if lib.is_file() {
            return Some(lib);
        }
    }
    None
}
