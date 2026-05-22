# Build Arqma MinGW wallet_merged + arqma-wallet-flutter-ffi.dll (native wallet FFI only).
# Prereqs: MSYS2 MINGW64 packages per .github/workflows/release.yml, rustup target x86_64-pc-windows-gnu.
param(
    [string]$MsysRoot = "C:\msys64",
    [switch]$SkipArqmaCMake
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$rustRoot = Join-Path $repoRoot "rust"

$env:Path = "$MsysRoot\mingw64\bin;$MsysRoot\usr\bin;" + $env:Path
$env:ARQMA_WALLET2_MSYS_ROOT = "$MsysRoot\mingw64"
$env:ARQMA_MINGW_BIN = "$MsysRoot\mingw64\bin"
$env:ARQMA_WALLET2_UPSTREAM_DIR = Join-Path $rustRoot "arqma-rpc-upstream"
if (-not $env:CARGO_PROFILE_RELEASE_LTO) { $env:CARGO_PROFILE_RELEASE_LTO = "thin" }
$env:ARQMA_WALLET_FFI_STATIC_HYBRID = "1"
$env:ARQMA_WALLET_FFI_USE_DEPENDS = "1"

if (-not $SkipArqmaCMake) {
    $bash = Join-Path $MsysRoot "usr\bin\bash.exe"
    if (-not (Test-Path $bash)) { throw "MSYS2 bash not found at $bash" }
    & $bash -lc "export ARQMA_WALLET2_UPSTREAM_DIR='$(($env:ARQMA_WALLET2_UPSTREAM_DIR) -replace '\\','/')'; bash '$($repoRoot -replace '\\','/')/build/ci/clone-arqma.sh' && bash '$($repoRoot -replace '\\','/')/build/ci/build-arqma-mingw.sh'"
    if ($LASTEXITCODE -ne 0) { throw "MinGW wallet_merged build failed (exit $LASTEXITCODE)" }
}

Push-Location $rustRoot
try {
    cargo build -p arqma-wallet-flutter-ffi --release --target x86_64-pc-windows-gnu
    if ($LASTEXITCODE -ne 0) { throw "cargo build arqma-wallet-flutter-ffi failed (exit $LASTEXITCODE)" }
} finally {
    Pop-Location
}

$dll = Join-Path $rustRoot "target\x86_64-pc-windows-gnu\release\arqma_wallet_flutter_ffi.dll"
if (-not (Test-Path $dll)) { throw "Missing $dll" }
Write-Host "OK: $dll"
