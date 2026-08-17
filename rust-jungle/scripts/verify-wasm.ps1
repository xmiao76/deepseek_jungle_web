# Reproducibility gate: rebuilds jungle_wasm from source and diffs it against
# the committed wwwroot/js/rust-engine.wasm. Requires the pinned toolchain
# (rust-toolchain.toml) and the wasm32-unknown-unknown target.
#
#   pwsh -File rust-jungle/scripts/verify-wasm.ps1

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot) # repo root

Push-Location (Join-Path $root "rust-jungle")
try {
    cargo build --release -p jungle-wasm --target wasm32-unknown-unknown
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed." }
}
finally {
    Pop-Location
}

$built = Join-Path $root "rust-jungle\target\wasm32-unknown-unknown\release\jungle_wasm.wasm"
$committed = Join-Path $root "src\JungleGame.Web\wwwroot\js\rust-engine.wasm"

if (-not (Test-Path $committed)) { throw "Committed rust-engine.wasm missing." }

$builtHash = (Get-FileHash $built -Algorithm SHA256).Hash
$committedHash = (Get-FileHash $committed -Algorithm SHA256).Hash

if ($builtHash -ne $committedHash) {
    Write-Error "rust-engine.wasm is stale: rebuild with `cargo build --release -p jungle-wasm --target wasm32-unknown-unknown` and copy the result to src/JungleGame.Web/wwwroot/js/rust-engine.wasm."
    exit 1
}

Write-Host "rust-engine.wasm is reproducible (SHA-256 $committedHash)."
