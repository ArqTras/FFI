use std::path::{Path, PathBuf};

#[cfg(target_os = "windows")]
fn newest_subdir(root: &Path) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root)
        .ok()?
        .filter_map(|e| e.ok().map(|x| x.path()))
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    dirs.pop()
}

fn msvc_sdk_include_dirs() -> Vec<PathBuf> {
    #[cfg(not(target_os = "windows"))]
    {
        Vec::new()
    }
    #[cfg(target_os = "windows")]
    {
        // Make MSVC wallet2 build independent from Developer Prompt.
        // We add common MSVC/Windows SDK include locations explicitly.
        let mut out = Vec::new();
        if let Some(vc_tools) = newest_subdir(Path::new(
            r"C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC",
        )) {
            out.push(vc_tools.join("include"));
        }
        if let Some(win10_inc_ver) =
            newest_subdir(Path::new(r"C:\Program Files (x86)\Windows Kits\10\Include"))
        {
            out.push(win10_inc_ver.join("ucrt"));
            out.push(win10_inc_ver.join("shared"));
            out.push(win10_inc_ver.join("um"));
            out.push(win10_inc_ver.join("winrt"));
            out.push(win10_inc_ver.join("cppwinrt"));
        }
        out
    }
}

/// MinGW g++ for `windows-gnu` Rust target (cxx-build). CI may use a non-default MSYS2 root.
fn resolve_mingw_gxx_exe() -> Option<String> {
    if let Ok(explicit) = std::env::var("ARQMA_WALLET2_GXX") {
        let p = Path::new(explicit.trim());
        if p.is_file() {
            return Some(p.to_string_lossy().into_owned());
        }
    }
    if let Ok(root) = std::env::var("ARQMA_WALLET2_MSYS_ROOT") {
        let p = Path::new(root.trim())
            .join("bin")
            .join("x86_64-w64-mingw32-g++.exe");
        if p.is_file() {
            return Some(p.to_string_lossy().into_owned());
        }
    }
    let legacy = Path::new(r"C:\msys64\mingw64\bin\x86_64-w64-mingw32-g++.exe");
    if legacy.is_file() {
        return Some(legacy.to_string_lossy().into_owned());
    }
    None
}

fn main() {
    let manifest_dir =
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let rust_workspace = manifest_dir
        .parent()
        .expect("arqma-wallet2-api crate must live under rust/");
    let upstream_path = match std::env::var("ARQMA_WALLET2_UPSTREAM_DIR") {
        Ok(p) if !p.trim().is_empty() => {
            let pb = PathBuf::from(p.trim());
            if pb.is_absolute() {
                pb
            } else {
                // `rust/.cargo/config.toml` `[env]` paths are relative to the directory that contains
                // `.cargo/` (i.e. `rust/`), not the package manifest dir.
                rust_workspace.join(pb)
            }
        }
        _ => rust_workspace.join("arqma-rpc-upstream"),
    };
    let api_dir = upstream_path.join("src").join("wallet").join("api");
    let upstream_src_dir = upstream_path.join("src");
    let header = api_dir.join("wallet2_api.h");
    if !header.is_file() {
        panic!(
            "arqma-wallet2-api: expected Arqma core headers at {} (missing {}).\n\
       From the `rust/` directory in this repo, run for example:\n\
         git clone -b pospow https://github.com/arqtras/arqma.git arqma-rpc-upstream\n\
       so that `rust/arqma-rpc-upstream/src/wallet/api/wallet2_api.h` exists, or set\n\
       ARQMA_WALLET2_UPSTREAM_DIR to the root of your Arqma core checkout.\n\
       See rust/docs/NATIVE_WALLET2.md",
            api_dir.display(),
            header.display()
        );
    }

    // `exportPendingRelaySlices` + `relayTxFromMetadataHex` use the same portable_binary `pending_tx`
    // encoding. That is **not** the same as `PendingTransaction::commit(filename, true)` file bytes
    // used by the legacy hex export path — relay would always fail to deserialize without export.
    //
    // `destinationAmountsPerSlice` on `PendingTransaction` is optional UI metadata only; do not gate
    // the export path on it (Arqma headers may omit the symbol while export/relay still exist).
    let header_text = std::fs::read_to_string(&header).unwrap_or_default();
    let has_relay_from_hex = header_text.contains("relayTxFromMetadataHex");
    let has_export_pending_relay = header_text.contains("exportPendingRelaySlices")
        && has_relay_from_hex;
    let has_destination_amounts_per_slice = header_text.contains("destinationAmountsPerSlice");
    let has_slice_relay = has_export_pending_relay && has_destination_amounts_per_slice;
    let has_register_service_node = header_text.contains("registerServiceNode");

    println!("cargo:rerun-if-env-changed=ARQMA_WALLET2_UPSTREAM_DIR");
    println!("cargo:rerun-if-changed=src/native.rs");
    println!("cargo:rerun-if-changed=src/wallet2_api_wrapper.cpp");
    println!("cargo:rerun-if-changed=src/wallet2_api_wrapper.hpp");
    println!(
        "cargo:rerun-if-changed={}",
        api_dir.join("wallet2_api.h").display()
    );
    println!("cargo:rerun-if-env-changed=ARQMA_WALLET2_LIB_DIR");
    println!("cargo:rerun-if-env-changed=ARQMA_WALLET2_LIB_NAME");
    println!("cargo:rerun-if-env-changed=ARQMA_WALLET2_MSYS_ROOT");
    println!("cargo:rerun-if-env-changed=ARQMA_WALLET2_GXX");
    println!("cargo:rerun-if-env-changed=ARQMA_WALLET_FFI_STATIC_HYBRID");
    println!("cargo:rerun-if-env-changed=ARQMA_WALLET_FFI_USE_DEPENDS");

    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let mut b = cxx_build::bridge("src/native.rs");
    // GitHub-hosted Windows often installs MSYS2 under RUNNER_TEMP, not `C:\msys64`. Only pass an
    // explicit g++ path when we know the file exists; otherwise rely on PATH (cxx-build default).
    if target_os == "windows" && target_env == "gnu" {
        if let Some(compiler) = resolve_mingw_gxx_exe() {
            b.compiler(compiler);
        }
        // patch-arqma-mingw-gui.js adds daemonizer to wallet_merged; do not also emit check_admin stub.
        b.define("ARQMA_WALLET2_DAEMONIZER_IN_MERGED", "1");
    }
    if has_export_pending_relay {
        b.define("ARQMA_WALLET2_HAS_EXPORT_PENDING_RELAY", "1");
    }
    if has_slice_relay {
        b.define("ARQMA_WALLET2_HAS_SLICE_RELAY", "1");
    }
    if has_relay_from_hex {
        b.define("ARQMA_WALLET2_HAS_RELAY_FROM_HEX", "1");
    }
    if has_register_service_node {
        b.define("ARQMA_WALLET2_HAS_REGISTER_SERVICE_NODE", "1");
    }
    b.file("src/wallet2_api_wrapper.cpp")
        .include("src")
        .include(&upstream_src_dir)
        .include(api_dir)
        .flag_if_supported("-std=c++17");
    if target_env == "msvc" {
        for inc in msvc_sdk_include_dirs() {
            b.include(inc);
        }
    }
    b.compile("arqma_wallet2_api_bridge");

    if target_os == "windows" {
        configure_wallet2_linking(&upstream_path, &target_env);
    } else if target_os == "macos" {
        configure_wallet2_linking_macos(&upstream_path);
    } else if target_os == "linux" || target_os == "android" {
        configure_wallet2_linking_linux(&upstream_path);
    } else if target_os == "ios" {
        configure_wallet2_linking_ios(&upstream_path);
    }
}

/// When building `arqma-wallet-flutter-ffi` with hybrid static deps, that crate emits `-Wl,-Bstatic`
/// groups and ICU as dynamic; skip duplicate `rustc-link-lib=dylib` lines here.
fn wallet_ffi_static_hybrid() -> bool {
    std::env::var("ARQMA_WALLET_FFI_STATIC_HYBRID")
        .map(|v| matches!(v.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

fn find_wallet_merged_dir(upstream: &Path) -> Option<PathBuf> {
    for root in [
        upstream.join("build/ci-depends-release"),
        upstream.join("build-ios-device"),
        upstream.join("build-ios-sim"),
        upstream.join("build-android-depends-aarch64-linux-android"),
        upstream.join("build-android-depends-x86_64-linux-android"),
        upstream.join("build-android-depends-armv7-linux-androideabi"),
        upstream.join("build"),
        upstream.join("build-mingw"),
    ] {
        if let Some(p) = find_wallet_merged_under(&root) {
            return Some(p);
        }
    }
    None
}

/// iOS: search path for `libwallet_merged.a` only (fat archive; no Homebrew dylibs).
fn configure_wallet2_linking_ios(upstream_path: &Path) {
    let lib_dir = std::env::var("ARQMA_WALLET2_LIB_DIR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| find_wallet_merged_dir(upstream_path));

    let Some(lib_dir) = lib_dir else {
        println!(
            "cargo:warning=arqma-wallet2-api (iOS): build `wallet_merged` for iOS (rust/tool/build_ios_wallet_merged.sh) or set ARQMA_WALLET2_LIB_DIR."
        );
        return;
    };

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    let depends_lib = upstream_path.join("contrib/depends/aarch64-apple-ios/lib");
    if depends_lib.is_dir() {
        println!("cargo:rustc-link-search=native={}", depends_lib.display());
    }
    if let Ok(extra) = std::env::var("ARQMA_WALLET_FFI_DEPENDS_LIB_DIR") {
        if !extra.trim().is_empty() {
            println!("cargo:rustc-link-search=native={}", extra.trim());
        }
    }
    if let Some(cmake_binary) = lib_dir.parent().and_then(|p| p.parent()) {
        for d in [
            "contrib/epee/src",
            "external/easylogging++",
            "external/randomarq",
            "src/lmdb/liblmdb",
            "src/cryptonote_basic",
        ] {
            let p = cmake_binary.join(d);
            if p.is_dir() {
                println!("cargo:rustc-link-search=native={}", p.display());
            }
        }
    }
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

fn find_wallet_merged_under(root: &Path) -> Option<PathBuf> {
    if !root.is_dir() {
        return None;
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if dir.join("libwallet_merged.a").is_file() || dir.join("wallet_merged.lib").is_file() {
            return Some(dir);
        }
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                }
            }
        }
    }
    None
}

fn brew_prefix() -> PathBuf {
    std::env::var_os("HOMEBREW_PREFIX")
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .unwrap_or_else(|| PathBuf::from("/opt/homebrew"))
}

/// Linux/macOS: `contrib/depends` + static-hybrid — do not add distro/Homebrew `-L` paths that shadow
/// the vendored static archives (e.g. Ubuntu `libboost_*.a` in `/usr` breaks PIC when linking `.so`).
fn wallet_ffi_depends_vendor_paths_suppressed() -> bool {
    wallet_ffi_static_hybrid()
        && std::env::var("ARQMA_WALLET_FFI_USE_DEPENDS")
            .map(|v| matches!(v.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
            .unwrap_or(false)
}

fn configure_wallet2_linking_macos(upstream_path: &Path) {
    let lib_dir = std::env::var("ARQMA_WALLET2_LIB_DIR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| find_wallet_merged_dir(upstream_path));

    let Some(lib_dir) = lib_dir else {
        println!("cargo:warning=arqma-wallet2-api (macOS): build upstream with `-D BUILD_GUI_DEPS=ON`, run `cmake --build . --target wallet_merged`, or set ARQMA_WALLET2_LIB_DIR to the folder containing libwallet_merged.a.");
        return;
    };

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    if let Some(cmake_binary) = lib_dir.parent().and_then(|p| p.parent()) {
        for d in [
            "contrib/epee/src",
            "external/easylogging++",
            "external/randomarq",
            "src/lmdb/liblmdb",
            "src/cryptonote_basic",
        ] {
            let p = cmake_binary.join(d);
            if p.is_dir() {
                println!("cargo:rustc-link-search=native={}", p.display());
            }
        }
    }

    let bp = brew_prefix();
    println!("cargo:rustc-link-arg=-Wl,-search_paths_first,-headerpad_max_install_names");
    if wallet_ffi_depends_vendor_paths_suppressed() {
        // Boost/OpenSSL/etc. come from `contrib/depends` via explicit `.a` paths in `arqma-wallet-flutter-ffi`;
        // keep Homebrew search only for ICU (often not folded into the vendored set).
        for rel in ["opt/icu4c/lib", "opt/icu4c@78/lib"] {
            let p = bp.join(rel);
            if p.is_dir() {
                println!("cargo:rustc-link-search=native={}", p.display());
            }
        }
    } else {
        for rel in [
            "lib",
            "opt/openssl@3/lib",
            "opt/boost/lib",
            "opt/libsodium/lib",
            "opt/unbound/lib",
            "opt/readline/lib",
            "opt/hidapi/lib",
            "opt/icu4c/lib",
            "opt/icu4c@78/lib",
            "opt/lmdb/lib",
            "opt/zeromq/lib",
        ] {
            let p = bp.join(rel);
            if p.is_dir() {
                println!("cargo:rustc-link-search=native={}", p.display());
            }
        }
    }

    if !wallet_ffi_static_hybrid() {
        for lib in [
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
            "icuuc",
            "icui18n",
            "icudata",
            "z",
        ] {
            println!("cargo:rustc-link-lib=dylib={lib}");
        }
    }

    if !wallet_ffi_static_hybrid() {
        println!("cargo:rustc-link-lib=framework=AppKit");
        println!("cargo:rustc-link-lib=framework=IOKit");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
    }
}

fn configure_wallet2_linking_linux(upstream_path: &Path) {
    let lib_dir = std::env::var("ARQMA_WALLET2_LIB_DIR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| find_wallet_merged_dir(upstream_path));

    let Some(lib_dir) = lib_dir else {
        println!("cargo:warning=arqma-wallet2-api (Linux): build upstream with `-D BUILD_GUI_DEPS=ON`, target `wallet_merged`, or set ARQMA_WALLET2_LIB_DIR.");
        return;
    };

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    if let Some(cmake_binary) = lib_dir.parent().and_then(|p| p.parent()) {
        for d in [
            "contrib/epee/src",
            "external/easylogging++",
            "external/randomarq",
            "src/lmdb/liblmdb",
            "src/cryptonote_basic",
        ] {
            let p = cmake_binary.join(d);
            if p.is_dir() {
                println!("cargo:rustc-link-search=native={}", p.display());
            }
        }
    }

    if !wallet_ffi_depends_vendor_paths_suppressed() {
        for dir in [
            "/usr/lib/x86_64-linux-gnu",
            "/usr/lib",
            "/lib/x86_64-linux-gnu",
        ] {
            if Path::new(dir).is_dir() {
                println!("cargo:rustc-link-search=native={dir}");
            }
        }
    }

    if !wallet_ffi_static_hybrid() {
        for lib in [
            "hidapi-libusb",
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
            "icuuc",
            "icui18n",
            "icudata",
            "z",
            "dl",
            "pthread",
        ] {
            println!("cargo:rustc-link-lib=dylib={lib}");
        }
    }

    // Duplicate `readline_buffer.cpp.o` across static archives: resolved at the **final** artifact
    // link (see `rust/tauri-app/src-tauri/build.rs` `cargo:rustc-link-arg=-Wl,-z,muldefs`).
}

fn configure_wallet2_linking(upstream_path: &Path, target_env: &str) {
    // Allow explicit override from environment.
    if let Ok(lib_dir) = std::env::var("ARQMA_WALLET2_LIB_DIR") {
        if !lib_dir.trim().is_empty() {
            let lib_name = std::env::var("ARQMA_WALLET2_LIB_NAME")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "wallet_merged".to_string());
            println!("cargo:rustc-link-search=native={lib_dir}");
            // windows-gnu: `wallet_merged` uses `#[link(..., modifiers = "+whole-archive")]` in
            // `src/lib.rs`; `rustc-link-lib=static=wallet_merged` here triggers "overriding linking
            // modifiers from command line is not supported".
            if target_env != "gnu" || lib_name != "wallet_merged" {
                println!("cargo:rustc-link-lib=static={lib_name}");
            }
            add_wallet2_external_libs(target_env);
            return;
        }
    }

    // Same strategy as macOS/Linux: search `build/` and `build-mingw/` for `libwallet_merged.a`
    // (MinGW) or `wallet_merged.lib` (MSVC).
    if let Some(lib_dir) = find_wallet_merged_dir(upstream_path) {
        println!("cargo:rustc-link-search=native={}", lib_dir.display());
        add_wallet2_external_libs(target_env);
        return;
    }

    println!("cargo:warning=wallet2 merged library not linked automatically. Build upstream target `wallet_merged`, or set ARQMA_WALLET2_LIB_DIR (and optional ARQMA_WALLET2_LIB_NAME).");
}

fn add_wallet2_external_libs(target_env: &str) {
    if target_env == "gnu" {
        let msys_root = std::env::var("ARQMA_WALLET2_MSYS_ROOT")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| r"C:\msys64\mingw64".to_string());
        println!("cargo:rustc-link-search=native={}\\lib", msys_root);

        // Boost / OpenSSL / ICU / Win32 — emit from `rust/tauri-app/src-tauri/build.rs` on windows-gnu
        // so flags sit **after** `#[link]` whole-archive `wallet_merged` in `lib.rs`.
        // Otherwise GNU ld processes Boost before `libwallet_merged.a`, drops unused objects, then
        // linking fails with undefined refs to `boost::serialization`, `boost::locale`, etc.
        // Native `epee` / `easylogging` / `randomx` live inside `wallet_merged` (upstream CMake fat archive).
    }
}
