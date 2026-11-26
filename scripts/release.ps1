param(
    [Parameter(Mandatory = $true)][string]$Version,
    # Short summary shown in the in-app update popup and at the top of the release.
    [Parameter(Mandatory = $true)][string]$Notes,
    # Markdown for the release's "What's new" section; defaults to the commit subjects since the last release.
    [string]$ChangesFile = "",
    [switch]$SkipChecks,
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repoRoot = Split-Path -Parent $PSScriptRoot
$repo = "HappyRave/gitgud"
$utf8 = [Text.UTF8Encoding]::new($false)
$versionFiles = @(
    "package.json",
    "src-tauri/tauri.conf.json",
    "src-tauri/Cargo.toml",
    "crates/gitgud-core/Cargo.toml",
    "crates/gitgud-cli/Cargo.toml"
)

function Invoke-Native([string]$what, [scriptblock]$run) {
    Write-Host "== $what"
    & $run
    if ($LASTEXITCODE -ne 0) {
        throw "$what failed (exit $LASTEXITCODE)"
    }
}

function Read-Text([string]$path) {
    return [IO.File]::ReadAllText((Join-Path $repoRoot $path))
}

Push-Location $repoRoot
$bumped = $false
try {
    # -- Preflight
    if ($Version -notmatch '^\d+\.\d+\.\d+$') {
        throw "Version must look like 1.2.3, got '$Version'"
    }
    $current = [string](Read-Text "src-tauri/tauri.conf.json" | ConvertFrom-Json).version
    if ([version]$Version -le [version]$current) {
        throw "Version $Version is not newer than the current $current"
    }
    if ($ChangesFile -and -not (Test-Path $ChangesFile)) {
        throw "No changes file at $ChangesFile"
    }
    $branch = git rev-parse --abbrev-ref HEAD
    if ($branch -ne "main") {
        throw "Release from main, not '$branch'"
    }
    if (git status --porcelain) {
        throw "The working tree isn't clean; commit or stash first"
    }
    Invoke-Native "git fetch" { git fetch --quiet --tags origin }
    if ((git rev-parse HEAD) -ne (git rev-parse origin/main)) {
        throw "main isn't in sync with origin/main; pull or push first"
    }
    if (git tag --list "v$Version") {
        throw "Tag v$Version already exists"
    }
    Invoke-Native "gh auth" { gh auth status --hostname github.com *> $null }
    if (-not $env:TAURI_SIGNING_PRIVATE_KEY -and -not (Test-Path (Join-Path $env:USERPROFILE ".tauri\gitgud.key"))) {
        throw "No updater signing key at %USERPROFILE%\.tauri\gitgud.key (or TAURI_SIGNING_PRIVATE_KEY)"
    }
    foreach ($file in $versionFiles) {
        if ((Read-Text $file) -notmatch "(?m)^\s*`"?version`"?\s*[:=]\s*`"$([regex]::Escape($current))`"") {
            throw "$file doesn't hold version $current"
        }
    }
    $previousTag = git describe --tags --abbrev=0 --match "v*" 2>$null
    Write-Host "Releasing $current -> $Version (previous tag: $previousTag)"
    if ($DryRun) {
        Write-Host "Dry run: preflight passed, nothing changed."
        return
    }

    # -- Checks
    if (-not $SkipChecks) {
        Invoke-Native "cargo fmt" { cargo fmt --all --check }
        Invoke-Native "cargo clippy" { cargo clippy --workspace --all-targets -- -D warnings }
        Invoke-Native "cargo test" { cargo test --workspace }
        Invoke-Native "svelte-check" { pnpm check }
        Invoke-Native "vitest" { pnpm vitest run }
    }

    # -- Bump (only the first version line of each file: the package's own)
    foreach ($file in $versionFiles) {
        $text = Read-Text $file
        $pattern = "(?m)^(\s*`"?version`"?\s*[:=]\s*`")$([regex]::Escape($current))`""
        $text = ([regex]$pattern).Replace($text, "`${1}$Version`"", 1)
        [IO.File]::WriteAllText((Join-Path $repoRoot $file), $text, $utf8)
    }
    $bumped = $true
    Invoke-Native "cargo update (workspace versions)" { cargo update --workspace --quiet }

    # -- Build
    Invoke-Native "installer + latest.json" {
        powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "package.ps1") -Notes $Notes
    }
    Invoke-Native "portable" {
        powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "package-portable.ps1")
    }
    $nsis = Join-Path $repoRoot "target\release\bundle\nsis"
    $portable = Join-Path $repoRoot "target\release\bundle\portable"
    $setup = Join-Path $nsis "gitgud_${Version}_x64-setup.exe"
    $assets = @(
        @{ Path = $setup; About = "Per-user installer (``gitgud`` + ``gitgud-cli`` on user ``PATH`` + bundled MinGit & git-lfs fallback). **Updates itself.**" },
        @{ Path = (Join-Path $portable "gitgud_${Version}_x64-portable.zip"); About = "Portable folder with bundled MinGit & git-lfs; extract and run. Tells you about updates and links here." },
        @{ Path = (Join-Path $portable "gitgud_${Version}_x64.exe"); About = "Standalone executable (needs system Git ``>= 2.45.2`` on ``PATH``)" }
    )
    $extra = @("$setup.sig", (Join-Path $nsis "latest.json"))
    foreach ($a in @($assets.Path) + $extra) {
        if (-not (Test-Path $a)) {
            throw "Missing release file $a"
        }
    }

    # -- Release notes
    $rows = foreach ($a in $assets) {
        $item = Get-Item $a.Path
        $hash = (Get-FileHash $a.Path -Algorithm SHA256).Hash.ToLowerInvariant()
        "| ``$($item.Name)`` | $($item.Length.ToString('N0', [Globalization.CultureInfo]::InvariantCulture)) B | $($a.About) | ``$hash`` |"
    }
    if ($ChangesFile) {
        $changes = [IO.File]::ReadAllText((Resolve-Path $ChangesFile))
    } elseif ($previousTag) {
        $changes = (git log --reverse --format="- %s" "$previousTag..HEAD") -join "`n"
    } else {
        $changes = "- First release."
    }
    $checks = if ($SkipChecks) { "Checks were skipped for this release." } else {
        "``cargo fmt`` / ``clippy -D warnings`` / ``cargo test --workspace``, ``svelte-check`` and ``vitest run`` passed."
    }
    $body = @"
# gitgud v$Version

$Notes

## Downloads (Windows x64)

| Package | Size | Contents & Notes | SHA-256 |
| --- | ---: | --- | --- |
$($rows -join "`n")

``latest.json`` and ``gitgud_${Version}_x64-setup.exe.sig`` are the in-app updater's manifest and signature.

## What's New Since $(if ($previousTag) { $previousTag } else { "the start" })

$changes

## Verification

$checks

The executables are not Authenticode-signed (Windows SmartScreen prompt expected on first launch).
"@
    $bodyPath = Join-Path $nsis "release-notes.md"
    [IO.File]::WriteAllText($bodyPath, $body, $utf8)

    # -- Commit, push, publish
    Invoke-Native "git commit" { git add -- $versionFiles Cargo.lock; git commit --quiet -m "Release v$Version" }
    $bumped = $false
    Invoke-Native "git push" { git push --quiet origin main }
    $sha = git rev-parse HEAD
    Invoke-Native "gh release create" {
        gh release create "v$Version" -R $repo --target $sha --title "gitgud v$Version" --latest `
            --notes-file $bodyPath @(@($assets.Path) + $extra)
    }

    # -- Verify what installed copies will see
    $manifest = Invoke-RestMethod "https://github.com/$repo/releases/latest/download/latest.json"
    if ($manifest.version -ne $Version) {
        throw "The update endpoint serves $($manifest.version), not $Version"
    }
    Write-Host "Released v${Version}: https://github.com/$repo/releases/tag/v$Version"
} catch {
    if ($bumped) {
        Write-Host "Restoring the version files after the failure."
        git checkout -- $versionFiles Cargo.lock
    }
    throw
} finally {
    Pop-Location
}
