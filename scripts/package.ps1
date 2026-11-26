param(
    [string]$Notes = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    if (-not $env:TAURI_SIGNING_PRIVATE_KEY) {
        $keyPath = Join-Path $env:USERPROFILE ".tauri\gitgud.key"
        if (-not (Test-Path $keyPath)) {
            throw "No updater signing key: set TAURI_SIGNING_PRIVATE_KEY or put the key at $keyPath"
        }
        $env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content -Path $keyPath -Raw).Trim()
    }
    if ($null -eq $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD) {
        $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""
    }

    & (Join-Path $PSScriptRoot "prepare-bundle.ps1")
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    pnpm exec tauri build --bundles nsis --config src-tauri/tauri.bundle.conf.json
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    $conf = Get-Content -Path (Join-Path $repoRoot "src-tauri\tauri.conf.json") -Raw | ConvertFrom-Json
    $version = [string]$conf.version
    $setupName = "gitgud_${version}_x64-setup.exe"
    $nsisDir = Join-Path $repoRoot "target\release\bundle\nsis"
    $sigPath = Join-Path $nsisDir "$setupName.sig"
    if (-not (Test-Path $sigPath)) {
        throw "Missing updater signature $sigPath"
    }

    # The updater reads this from the latest GitHub release; upload it next to the installer.
    $manifest = [ordered]@{
        version   = $version
        notes     = $Notes
        pub_date  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
        platforms = [ordered]@{
            "windows-x86_64" = [ordered]@{
                signature = (Get-Content -Path $sigPath -Raw).Trim()
                url       = "https://github.com/HappyRave/gitgud/releases/download/v$version/$setupName"
            }
        }
    }
    $manifestPath = Join-Path $nsisDir "latest.json"
    [IO.File]::WriteAllText($manifestPath, ($manifest | ConvertTo-Json -Depth 5), [Text.UTF8Encoding]::new($false))
    Write-Host "Wrote $manifestPath"
} finally {
    Pop-Location
}
