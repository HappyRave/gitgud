$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    & (Join-Path $PSScriptRoot "fetch-bundled-git.ps1")
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    Write-Host "Building gitgud-cli (release)..."
    cargo build --release -p gitgud-cli
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    $binDir = Join-Path $repoRoot "src-tauri\binaries"
    New-Item -ItemType Directory -Path $binDir -Force | Out-Null

    $srcExe = Join-Path $repoRoot "target\release\gitgud-cli.exe"
    $destExe = Join-Path $binDir "gitgud-cli-x86_64-pc-windows-msvc.exe"
    Copy-Item -Path $srcExe -Destination $destExe -Force
    Write-Host "Staged CLI sidecar at $destExe"
} finally {
    Pop-Location
}
