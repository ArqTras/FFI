//! Locating **executables** produced by upstream [arqma/arqma](https://github.com/arqma/arqma)
//! (`make release` → `build/release/bin/`, or `make install` → `$prefix/bin/`).
//!
//! Those binaries are the linked output of Arqma’s internal libraries (e.g. merged wallet code);
//! the GUI uses them as subprocesses and speaks JSON-RPC to `arqma-wallet-rpc`.

use std::path::{Path, PathBuf};

/// Which upstream-produced binary to resolve.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArqmaExecutableKind {
    /// `arqma-wallet-rpc` — HTTP JSON-RPC over wallet2.
    WalletRpc,
    /// `arqmad` full node.
    Daemon,
}

fn names(kind: ArqmaExecutableKind) -> (&'static str, &'static str) {
    match kind {
        ArqmaExecutableKind::WalletRpc => ("arqma-wallet-rpc.exe", "arqma-wallet-rpc"),
        ArqmaExecutableKind::Daemon => ("arqmad.exe", "arqmad"),
    }
}

fn direct_env(kind: ArqmaExecutableKind) -> &'static str {
    match kind {
        ArqmaExecutableKind::WalletRpc => "ARQMA_WALLET_RPC",
        ArqmaExecutableKind::Daemon => "ARQMA_DAEMON",
    }
}

fn pick_name<'a>(win: &'a str, unix: &'a str) -> &'a str {
    if cfg!(windows) {
        win
    } else {
        unix
    }
}

/// Locate an executable on `PATH` (`arqma-wallet-rpc.exe` / `arqmad.exe` on Windows).
pub fn find_in_path(win: &str, unix: &str) -> Option<PathBuf> {
    let name = pick_name(win, unix);
    let path_var = std::env::var_os("PATH")?;
    let sep = if cfg!(windows) { ';' } else { ':' };
    for dir in path_var.to_string_lossy().split(sep) {
        if dir.is_empty() {
            continue;
        }
        let p = PathBuf::from(dir.trim()).join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn bin_subdir(base: &Path) -> PathBuf {
    if base
        .file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("bin"))
    {
        base.to_path_buf()
    } else {
        base.join("bin")
    }
}

fn exe_in_bin_root(root: &str, win: &str, unix: &str) -> Option<PathBuf> {
    let base = PathBuf::from(root.trim());
    if base.as_os_str().is_empty() {
        return None;
    }
    let bin_dir = bin_subdir(&base);
    let name = pick_name(win, unix);
    let p = bin_dir.join(name);
    if p.is_file() {
        Some(p)
    } else {
        None
    }
}

/// Resolve `arqma-wallet-rpc` or `arqmad` using the same rules as the desktop shell.
///
/// Resolution order:
/// 1. Explicit path: `ARQMA_WALLET_RPC` or `ARQMA_DAEMON` (full path to the executable).
/// 2. `ARQMA_BUILD_DIR`: directory that contains `bin/` (typical: `…/arqma/build/release`).
/// 3. `ARQMA_INSTALL_PREFIX`: install prefix with `bin/` underneath (`make install`).
/// 4. `PATH`.
/// 5. Each path in `extra_candidates` that exists as a regular file (e.g. Tauri `resource_dir/bin`).
pub fn resolve_arqma_executable(
    kind: ArqmaExecutableKind,
    extra_candidates: impl IntoIterator<Item = PathBuf>,
) -> Option<PathBuf> {
    let (win, unix) = names(kind);
    if let Ok(v) = std::env::var(direct_env(kind)) {
        let pb = PathBuf::from(v.trim());
        if pb.is_file() {
            return Some(pb);
        }
    }
    if let Ok(bd) = std::env::var("ARQMA_BUILD_DIR") {
        if let Some(p) = exe_in_bin_root(&bd, win, unix) {
            return Some(p);
        }
    }
    if let Ok(px) = std::env::var("ARQMA_INSTALL_PREFIX") {
        if let Some(p) = exe_in_bin_root(&px, win, unix) {
            return Some(p);
        }
    }
    if let Some(p) = find_in_path(win, unix) {
        return Some(p);
    }
    extra_candidates.into_iter().find(|p| p.is_file())
}

/// See [`resolve_arqma_executable`].
pub fn resolve_wallet_rpc_path(
    extra_candidates: impl IntoIterator<Item = PathBuf>,
) -> Option<PathBuf> {
    resolve_arqma_executable(ArqmaExecutableKind::WalletRpc, extra_candidates)
}

/// See [`resolve_arqma_executable`].
pub fn resolve_daemon_path(extra_candidates: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    resolve_arqma_executable(ArqmaExecutableKind::Daemon, extra_candidates)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bin_subdir_trailing_bin() {
        let p = PathBuf::from("/opt/arqma/bin");
        assert_eq!(bin_subdir(&p), p);
    }

    #[test]
    fn bin_subdir_release() {
        let p = PathBuf::from("/opt/arqma/build/release");
        assert_eq!(
            bin_subdir(&p),
            PathBuf::from("/opt/arqma/build/release/bin")
        );
    }
}
