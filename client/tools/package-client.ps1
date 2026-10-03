<#
.SYNOPSIS
    Builds the Gravak client and lays out everything a player needs in
    dist\Gravak, then (when Inno Setup is installed) the GravakSetup.exe
    installer in dist.

.DESCRIPTION
    dist\Gravak\
        Gravak.exe        the client, with the C runtime linked in
        gravak.ini        game data folder and server
        game\             the game data the client reads (ani, c3, data, ini,
                          map, sound, c3.wdf, data.wdf)
        licenses\         LICENSE and NOTICE (Apache 2.0)

    Only the data folders are copied from the 5065 installation; its
    executables and libraries are not needed. Run again after a change:
    robocopy only copies what differs.

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

# 1. Build. Its own target folder keeps the static C runtime build from
#    rebuilding the everyday one, and the other way round.
Write-Host "Building the client (release)..." -ForegroundColor Cyan
$env:RUSTFLAGS = "-C target-feature=+crt-static"
$env:CARGO_TARGET_DIR = Join-Path $client "target\dist"
Push-Location $client
try {
    cargo build --release -p conquer-sandbox
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
} finally {
    Pop-Location
    Remove-Item Env:RUSTFLAGS, Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
}

# 2. The executable, its configuration and the licenses.
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

# 3. The game data: the folders and archives the client reads.
Write-Host "Copying the game data from $Assets..." -ForegroundColor Cyan
$game = Join-Path $package "game"
foreach ($folder in "ani", "c3", "data", "ini", "map", "sound") {
    robocopy (Join-Path $Assets $folder) (Join-Path $game $folder) /MIR /NJH /NJS /NFL /NDL /NP | Out-Null
    if ($LASTEXITCODE -ge 8) { throw "robocopy failed for $folder" }
}
robocopy $Assets $game "c3.wdf" "data.wdf" /NJH /NJS /NFL /NDL /NP | Out-Null
if ($LASTEXITCODE -ge 8) { throw "robocopy failed for the .wdf archives" }
$global:LASTEXITCODE = 0

$size = (Get-ChildItem $package -Recurse -File | Measure-Object Length -Sum).Sum / 1GB
Write-Host ("dist\Gravak ready: {0:N2} GB" -f $size) -ForegroundColor Green

# 4. The installer.
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
