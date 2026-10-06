<#
.SYNOPSIS
    Builds the Gravak client and lays out everything a player needs in
    dist\Gravak, then (when Inno Setup is installed) the GravakSetup.exe
    installer in dist.

.DESCRIPTION
    dist\Gravak\
        Gravak.exe        the client, with the C runtime linked in
        gravak.ini        game data folder and server
        game\gravak.gpk   all the game data, compressed and encrypted (ani,
                          c3, data, ini, map, sound, the entries of c3.wdf
                          and data.wdf, and our content\build)
        licenses\         LICENSE and NOTICE (Apache 2.0)

    Players get no loose game files: the client is built with the
    packed-only feature and reads nothing but game\gravak.gpk, whose
    entries it refuses when a byte was changed. The pack key lives in
    client\installer\pack.key (made on the first run, never committed);
    keep it to make packs for clients already handed out.

.EXAMPLE
    .\tools\package-client.ps1
    .\tools\package-client.ps1 -Assets D:\Conquer5065 -Auth 192.168.0.10:9958
#>
param(
    # The 5065 installation the game data is taken from.
    [string]$Assets = "R:\5065",
    # The login server written into gravak.ini.
    [string]$Auth = "127.0.0.1:9958",
    [string]$Realm = "Gravak",
    # Skip the installer even when Inno Setup is present.
    [switch]$NoInstaller
)

$ErrorActionPreference = "Stop"
$client = Split-Path -Parent $PSScriptRoot
$dist = Join-Path $client "dist"
$package = Join-Path $dist "Gravak"

if (-not (Test-Path (Join-Path $Assets "ini\GameMap.dat"))) {
    throw "No 5065 installation at '$Assets' (ini\GameMap.dat is missing). Pass -Assets <folder>."
}

# 1. The pack key, compiled into both the client and gravak-pack.
$keyFile = Join-Path $client "installer\pack.key"
if (-not (Test-Path $keyFile)) {
    $bytes = New-Object byte[] 32
    [System.Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($bytes)
    ($bytes | ForEach-Object { $_.ToString("x2") }) -join "" | Set-Content -NoNewline -Encoding ascii $keyFile
    Write-Host "New pack key: $keyFile (keep it, never commit it)" -ForegroundColor Yellow
}
$packKey = (Get-Content -Raw $keyFile).Trim()
if ($packKey -notmatch '^[0-9a-fA-F]{64}$') { throw "$keyFile must hold 64 hexadecimal digits" }

# 2. Our content (items, icons, map objects) built over the installation.
$projectRoot = Split-Path -Parent $client
$contentBuild = Join-Path $projectRoot "content\build\client"
& (Join-Path $projectRoot "build-content.ps1") -Assets $Assets
if (-not (Test-Path (Join-Path $contentBuild ".gravak-build"))) {
    throw "content\ could not be built; see the error above"
}

# 3. Build. Its own target folder keeps the static C runtime build from
#    rebuilding the everyday one, and the other way round.
Write-Host "Building the client (release)..." -ForegroundColor Cyan
$env:RUSTFLAGS = "-C target-feature=+crt-static"
$env:CARGO_TARGET_DIR = Join-Path $client "target\dist"
$env:GRAVAK_PACK_KEY = $packKey
Push-Location $client
try {
    cargo build --release -p conquer-sandbox --features packed-only
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
    cargo build --release -p gravak-content --bin gravak-pack
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed (gravak-pack)" }
} finally {
    Pop-Location
    Remove-Item Env:RUSTFLAGS, Env:CARGO_TARGET_DIR, Env:GRAVAK_PACK_KEY -ErrorAction SilentlyContinue
}

# 4. The executable, its configuration and the licenses.
New-Item -ItemType Directory -Force $package, (Join-Path $package "licenses") | Out-Null
Copy-Item (Join-Path $client "target\dist\release\conquer-sandbox.exe") (Join-Path $package "Gravak.exe") -Force
@"
; Gravak client settings.
; Assets: the game data folder, relative to this file.
; Auth:   the login server (host:port).
[Client]
Assets=game
Auth=$Auth
Realm=$Realm
"@ | Set-Content -Encoding utf8 (Join-Path $package "gravak.ini")
Copy-Item (Join-Path $client "LICENSE"), (Join-Path $client "NOTICE") (Join-Path $package "licenses") -Force

# 5. The game data: one encrypted pack. The game folder is made afresh,
#    so no loose file from an older package is left beside it.
Write-Host "Packing the game data from $Assets..." -ForegroundColor Cyan
$game = Join-Path $package "game"
if (Test-Path $game) { Remove-Item -Recurse -Force $game }
New-Item -ItemType Directory -Force $game | Out-Null
& (Join-Path $client "target\dist\release\gravak-pack.exe") --assets $Assets --content $contentBuild --out (Join-Path $game "gravak.gpk")
if ($LASTEXITCODE -ne 0) { throw "gravak-pack failed" }

$size = (Get-ChildItem $package -Recurse -File | Measure-Object Length -Sum).Sum / 1GB
Write-Host ("dist\Gravak ready: {0:N2} GB" -f $size) -ForegroundColor Green

# 6. The installer.
if ($NoInstaller) { return }
$iscc = @(
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $iscc) {
    Write-Host "Inno Setup not found: dist\Gravak is ready, but no installer was made." -ForegroundColor Yellow
    Write-Host "Install it with: winget install JRSoftware.InnoSetup" -ForegroundColor Yellow
    return
}
Write-Host "Building the installer..." -ForegroundColor Cyan
& $iscc /Q (Join-Path $client "installer\gravak.iss")
if ($LASTEXITCODE -ne 0) { throw "Inno Setup failed" }
Write-Host "Installer ready: dist\GravakSetup.exe" -ForegroundColor Green
