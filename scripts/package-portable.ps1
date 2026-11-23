param(
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    $conf = Get-Content -Path (Join-Path $repoRoot "src-tauri\tauri.conf.json") -Raw | ConvertFrom-Json
    $version = [string]$conf.version
    if ([string]::IsNullOrWhiteSpace($version)) {
        throw "Failed to read version from src-tauri/tauri.conf.json"
    }

    if (-not $SkipBuild) {
        & (Join-Path $PSScriptRoot "prepare-bundle.ps1")
        if ($LASTEXITCODE -ne 0) {
            exit $LASTEXITCODE
        }

        Write-Host "Building gitgud (release, no-bundle)..."
        if (Get-Command tauri -ErrorAction SilentlyContinue) {
            tauri build --no-bundle
        } elseif (Get-Command pnpm -ErrorAction SilentlyContinue) {
            pnpm exec tauri build --no-bundle
        } else {
            npx --yes pnpm@10 exec tauri build --no-bundle
        }
        if ($LASTEXITCODE -ne 0) {
            exit $LASTEXITCODE
        }
    }

    $releaseExe = Join-Path $repoRoot "target\release\gitgud.exe"
    $releaseCli = Join-Path $repoRoot "target\release\gitgud-cli.exe"
    $gitSrc = Join-Path $repoRoot "src-tauri\resources\git"
    foreach ($req in @($releaseExe, $releaseCli, $gitSrc)) {
        if (-not (Test-Path $req)) {
            throw "Required build artifact missing: $req"
        }
    }

    $outDir = Join-Path $repoRoot "target\release\bundle\portable"
    New-Item -ItemType Directory -Path $outDir -Force | Out-Null

    $folderName = "gitgud_${version}_x64"
    $stageDir = Join-Path $outDir $folderName
    $zipPath = Join-Path $outDir "${folderName}-portable.zip"
    $standalonePath = Join-Path $outDir "${folderName}.exe"

    if (Test-Path $stageDir) {
        Remove-Item -Path $stageDir -Recurse -Force
    }
    if (Test-Path $zipPath) {
        Remove-Item -Path $zipPath -Force
    }

    $gitDest = Join-Path $stageDir "resources\git"
    New-Item -ItemType Directory -Path $gitDest -Force | Out-Null

    Copy-Item -Path $releaseExe -Destination (Join-Path $stageDir "gitgud.exe") -Force
    Copy-Item -Path $releaseCli -Destination (Join-Path $stageDir "gitgud-cli.exe") -Force
    Copy-Item -Path (Join-Path $gitSrc "*") -Destination $gitDest -Recurse -Force

    Add-Type -AssemblyName System.IO.Compression.FileSystem
    [System.IO.Compression.ZipFile]::CreateFromDirectory(
        $stageDir,
        $zipPath,
        [System.IO.Compression.CompressionLevel]::Optimal,
        $true
    )

    Copy-Item -Path $releaseExe -Destination $standalonePath -Force
    Remove-Item -Path $stageDir -Recurse -Force

    $zipInfo = Get-Item $zipPath
    $exeInfo = Get-Item $standalonePath
    Write-Host "Portable zip: $($zipInfo.FullName) ($($zipInfo.Length) bytes)"
    Write-Host "Standalone exe: $($exeInfo.FullName) ($($exeInfo.Length) bytes)"
} finally {
    Pop-Location
}
