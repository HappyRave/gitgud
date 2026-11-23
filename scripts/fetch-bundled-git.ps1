param(
    [switch]$Force
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$minGitVersion = "2.56.1.windows.1"
$minGitFile = "MinGit-2.56.1-64-bit.zip"
$minGitUrl = "https://github.com/git-for-windows/git/releases/download/v$minGitVersion/$minGitFile"
$minGitSha256 = "4A92850B6E917DA4FD010669B614B55DCCF5B0EC333A534CC4BE4700DFEFDAFD"

$lfsVersion = "3.8.1"
$lfsFile = "git-lfs-windows-amd64-v$lfsVersion.zip"
$lfsUrl = "https://github.com/git-lfs/git-lfs/releases/download/v$lfsVersion/$lfsFile"
$lfsSha256 = "5BB771C12B3DACC535E912D0270A010C209D4DA2E63819C5918E3547C2DEFED9"

$repoRoot = Split-Path -Parent $PSScriptRoot
$targetDir = Join-Path $repoRoot "src-tauri\resources\git"
$markerPath = Join-Path $targetDir ".gitgud-bundle-version"
$expectedMarker = "mingit=$minGitVersion;git-lfs=$lfsVersion"
$gitExe = Join-Path $targetDir "cmd\git.exe"

if (-not $Force -and (Test-Path $markerPath) -and (Test-Path $gitExe)) {
    $currentMarker = (Get-Content $markerPath -Raw).Trim()
    if ($currentMarker -eq $expectedMarker) {
        Write-Host "Bundled Git ($expectedMarker) already staged at $targetDir"
        exit 0
    }
}

$tmpDir = Join-Path ([System.IO.Path]::GetTempPath()) ("gitgud-git-bundle-" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null

function Get-VerifiedArchive {
    param(
        [string]$Url,
        [string]$Dest,
        [string]$ExpectedHash
    )
    Write-Host "Downloading $Url..."
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    Invoke-WebRequest -Uri $Url -OutFile $Dest -UseBasicParsing
    $actual = (Get-FileHash -Path $Dest -Algorithm SHA256).Hash.ToUpperInvariant()
    if ($actual -ne $ExpectedHash.ToUpperInvariant()) {
        throw "SHA-256 mismatch for ${Url}: expected $ExpectedHash, got $actual"
    }
    Write-Host "Verified SHA-256 ($actual)"
}

try {
    $minGitZip = Join-Path $tmpDir $minGitFile
    $lfsZip = Join-Path $tmpDir $lfsFile
    $lfsExtract = Join-Path $tmpDir "lfs"

    Get-VerifiedArchive -Url $minGitUrl -Dest $minGitZip -ExpectedHash $minGitSha256
    Get-VerifiedArchive -Url $lfsUrl -Dest $lfsZip -ExpectedHash $lfsSha256

    if (Test-Path $targetDir) {
        Remove-Item -Path $targetDir -Recurse -Force
    }
    New-Item -ItemType Directory -Path $targetDir -Force | Out-Null

    Write-Host "Extracting $minGitFile to $targetDir..."
    Expand-Archive -Path $minGitZip -DestinationPath $targetDir -Force

    Write-Host "Extracting $lfsFile..."
    Expand-Archive -Path $lfsZip -DestinationPath $lfsExtract -Force
    $lfsExe = Get-ChildItem -Path $lfsExtract -Filter "git-lfs.exe" -Recurse | Select-Object -First 1
    if (-not $lfsExe) {
        throw "git-lfs.exe not found inside $lfsFile"
    }

    $cmdDir = Join-Path $targetDir "cmd"
    if (-not (Test-Path $cmdDir)) {
        throw "MinGit layout missing cmd directory at $cmdDir"
    }
    Copy-Item -Path $lfsExe.FullName -Destination (Join-Path $cmdDir "git-lfs.exe") -Force

    $gitCoreDir = Join-Path $targetDir "mingw64\libexec\git-core"
    if (Test-Path $gitCoreDir) {
        Copy-Item -Path $lfsExe.FullName -Destination (Join-Path $gitCoreDir "git-lfs.exe") -Force
    }

    Set-Content -Path $markerPath -Value $expectedMarker -NoNewline -Encoding UTF8

    $env:PATH = "$cmdDir;$env:PATH"
    $gitVerOut = & $gitExe --version
    $lfsVerOut = & $gitExe lfs version
    Write-Host "Smoke check passed: $gitVerOut | $lfsVerOut"
} finally {
    if (Test-Path $tmpDir) {
        Remove-Item -Path $tmpDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}
