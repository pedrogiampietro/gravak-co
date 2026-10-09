<#
.SYNOPSIS
    Releases the client to players in one go: builds it (package-client.ps1),
    lays out the signed update (publish-update.ps1), uploads to the VPS what
    changed and publishes it there with the new installer.

.DESCRIPTION
    Players' clients update themselves on their next start; the website's
    download serves the new GravakSetup.exe.

    Only the files whose size or SHA-256 differ from the live manifest
    (https://gravak.fun/update/manifest.txt) are uploaded, in one tar, with
    scp; on the VPS, gravak-publish-update checks every file against the new
    manifest before publishing, and puts manifest.sig and manifest.txt last.

    Needs OpenSSH (ssh, scp) and tar, which come with Windows 10 and 11.
    With an SSH key on the VPS (ssh-copy-id, or the key appended to
    /root/.ssh/authorized_keys) nothing asks for the password.

    Keep client\keys and client\installer\pack.key from one release to the
    next: clients already handed out take only updates signed with the same
    key.

.EXAMPLE
    .\tools\release-client.ps1
    .\tools\release-client.ps1 -Assets D:\Conquer5065
    .\tools\release-client.ps1 -SkipBuild -NoInstaller
#>
param(
    # The 5065 installation the game data is taken from.
    [string]$Assets = "",
    # The VPS, as ssh reaches it.
    [string]$Server = "root@187.127.15.162",
    # The login server written into gravak.ini.
    [string]$Auth = "187.127.15.162:9958",
    # Where players' clients update from (nginx serves it from /var/www/gravak-update).
    [string]$Update = "https://gravak.fun/update",
    # The VPS auth server's public key (/srv/gravak/keys/login.key's half).
    [string]$LoginKey = "b68f8abda61c1ecc7dce3dfee458ea8d4ab60c2e54807d53e2c44f4863122b7a",
    # Release what dist\Gravak holds, without building again.
    [switch]$SkipBuild,
    # Leave the source as it is instead of pulling main first.
    [switch]$SkipPull,
    # Publish the update only, not a new installer.
    [switch]$NoInstaller
)

$ErrorActionPreference = "Stop"
$client = Split-Path -Parent $PSScriptRoot
$dist = Join-Path $client "dist"
$updateDir = Join-Path $dist "update"
$started = Get-Date

foreach ($tool in "ssh", "scp", "tar") {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
        throw "$tool not found: install Windows' OpenSSH Client (Settings > Apps > Optional features)."
    }
}

function Step($text) { Write-Host "`n== $text" -ForegroundColor Cyan }

# 1. The source.
if (-not $SkipPull -and -not $SkipBuild) {
    Step "Pulling main"
    Push-Location (Split-Path -Parent $client)
    try {
        & git pull --ff-only
        if ($LASTEXITCODE -ne 0) { throw "git pull failed; commit or stash your changes, or run with -SkipPull." }
    } finally { Pop-Location }
}

# 2. The client and its installer.
if (-not $SkipBuild) {
    Step "Building the client"
    $package = @{ Auth = $Auth; Update = $Update; LoginKey = $LoginKey }
    if ($Assets) { $package.Assets = $Assets }
    if ($NoInstaller) { $package.NoInstaller = $true }
    & (Join-Path $PSScriptRoot "package-client.ps1") @package
}

# 3. The signed update.
Step "Signing the update"
& (Join-Path $PSScriptRoot "publish-update.ps1")

# 4. What changed since the live update.
Step "Comparing with $Update"
$live = @{}
try {
    $text = (Invoke-WebRequest -UseBasicParsing -Uri "$Update/manifest.txt" -TimeoutSec 30).Content
    if ($text -is [byte[]]) { $text = [Text.Encoding]::ASCII.GetString($text) }
    foreach ($line in $text -split "`n") {
        if ($line -match '^file=(.+)$') { $live[$Matches[1].Trim()] = $true }
    }
} catch {
    Write-Host "No live update yet: everything is uploaded." -ForegroundColor Yellow
}
$changed = @()
foreach ($line in (Get-Content (Join-Path $updateDir "manifest.txt"))) {
    if ($line -match '^file=(.+)$' -and -not $live.ContainsKey($Matches[1].Trim())) {
        $changed += $Matches[1].Trim().Split('|')[0]
    }
}
$changed | ForEach-Object { Write-Host "  $_" }
Write-Host ("{0} files changed" -f $changed.Count)

$list = Join-Path $dist "update-files.txt"
$tarball = Join-Path $dist "gravak-update.tar"
(@($changed) + "manifest.sig" + "manifest.txt") -join "`n" | Set-Content -NoNewline -Encoding ascii $list
if (Test-Path $tarball) { Remove-Item -Force $tarball }
& tar -cf $tarball -C $updateDir -T $list
if ($LASTEXITCODE -ne 0) { throw "tar failed" }

$uploads = @($tarball)
$remoteInstaller = ""
$installer = Join-Path $dist "GravakSetup.exe"
if (-not $NoInstaller) {
    if ((Test-Path $installer) -and (Get-Item $installer).LastWriteTime -ge $started) {
        $uploads += $installer
        $remoteInstaller = "/tmp/GravakSetup.exe"
    } elseif (-not $SkipBuild) {
        Write-Host "No new GravakSetup.exe (Inno Setup missing?): the website keeps its installer." -ForegroundColor Yellow
    } elseif (Test-Path $installer) {
        $uploads += $installer
        $remoteInstaller = "/tmp/GravakSetup.exe"
    }
}

# 5. Up to the VPS, then live.
Step "Uploading to $Server"
& scp @uploads "${Server}:/tmp/"
if ($LASTEXITCODE -ne 0) { throw "scp failed" }

Step "Publishing"
& ssh $Server "gravak-publish-update /tmp/gravak-update.tar $remoteInstaller"
if ($LASTEXITCODE -ne 0) { throw "Publishing failed on the VPS; nothing changed for players." }

Remove-Item -Force $tarball, $list
Write-Host "`nReleased. Clients update on their next start." -ForegroundColor Green
if ($remoteInstaller) {
    Write-Host "Upload dist\GravakSetup.exe to MediaFire too, for the mirror link." -ForegroundColor Yellow
}
