# Behavior-neutrality gate: a refactor that claims not to change search behavior
# must produce byte-identical fixed-depth node counts and best moves.
#
# Builds the base ref in a temporary worktree and the current working tree,
# runs the deterministic --dump-nodes mode on both (suite positions depths 1..8,
# seeded fuzz positions depths 1..4), and fails on any line difference.
#
#   powershell -File scripts/check-exact-nodes.ps1 [-BaseRef HEAD] [-Fuzz 200] [-Seed 20240816]
param(
    [string]$BaseRef = "HEAD",  # the pre-refactor commit (must already contain the --dump-nodes harness)
    [int]$Fuzz = 200,
    [int]$Seed = 20240816,
    [string]$WorkRoot = "$env:TEMP\jungle-exact-nodes"
)

$ErrorActionPreference = "Stop"

Remove-Item -Recurse -Force $WorkRoot -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Path $WorkRoot | Out-Null

# --- Base ref ---
$baseDir = Join-Path $WorkRoot "base"
git worktree add --detach $baseDir $BaseRef | Out-Null
try {
    dotnet build (Join-Path $baseDir "src\JungleGame.Bench\JungleGame.Bench.csproj") -c Release | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "Base ref build failed." }
    Push-Location (Join-Path $baseDir "src\JungleGame.Bench")
    dotnet run -c Release --no-build -- --bench --dump-nodes (Join-Path $WorkRoot "base.txt") --fuzz $Fuzz --seed $Seed | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "Base ref dump failed." }
    Pop-Location
} finally {
    git worktree remove --force $baseDir
}

# --- Working tree ---
dotnet build (Join-Path $PSScriptRoot "..\src\JungleGame.Bench\JungleGame.Bench.csproj") -c Release | Out-Null
if ($LASTEXITCODE -ne 0) { throw "Working-tree build failed." }
Push-Location (Join-Path $PSScriptRoot "..\src\JungleGame.Bench")
dotnet run -c Release --no-build -- --bench --dump-nodes (Join-Path $WorkRoot "new.txt") --fuzz $Fuzz --seed $Seed | Out-Null
if ($LASTEXITCODE -ne 0) { throw "Working-tree dump failed." }
Pop-Location

$diff = Compare-Object (Get-Content (Join-Path $WorkRoot "base.txt")) (Get-Content (Join-Path $WorkRoot "new.txt"))
if ($diff) {
    Write-Error "Node-count/best-move dump differs between $BaseRef and the working tree ($($diff.Count) differing lines). The refactor is not behavior-neutral."
    exit 1
}

Write-Host "Exact-equality gate PASSED: $BaseRef and the working tree produce identical fixed-depth node counts and best moves."
