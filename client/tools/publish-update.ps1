<#
.SYNOPSIS
    Lays out an update for players' clients: dist\update holds the files of
    dist\Gravak and the manifest.txt that lists them, ready to upload as
    they are to the folder named by -Update in package-client.ps1.

.DESCRIPTION
    Run package-client.ps1 first. Clients read manifest.txt at start-up and
    download every file whose size or SHA-256 differs from theirs, then
    restart on the new files (see crates\sandbox\src\updater.rs). Any web
    host that serves static files will do: a VPS with nginx, Cloudflare R2,
    an S3 bucket.

    manifest.txt is signed with the release key (client\keys\update.key,
    made by package-client.ps1) into manifest.sig: a release client takes
    no manifest without it. Upload manifest.sig and manifest.txt last.

    gravak.ini is left out: it is each install's own (game folder, update
    address), written by the installer.

.EXAMPLE
    .\tools\package-client.ps1 -Auth play.example.com:9958 -Update https://cdn.example.com/gravak
    .\tools\publish-update.ps1
#>
param(
    [string]$Package = (Join-Path (Split-Path -Parent $PSScriptRoot) "dist\Gravak"),
    [string]$Out = (Join-Path (Split-Path -Parent $PSScriptRoot) "dist\update")
)

$ErrorActionPreference = "Stop"
$client = Split-Path -Parent $PSScriptRoot
if (-not (Test-Path (Join-Path $Package "Gravak.exe"))) {
    throw "No package at '$Package'; run tools\package-client.ps1 first."
}
$version = (Select-String -Path (Join-Path $client "Cargo.toml") -Pattern '^version = "(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value

$signer = Join-Path $client "target\dist\release\gravak-sign.exe"
$secret = Join-Path $client "keys\update.key"
if (-not (Test-Path $signer) -or -not (Test-Path $secret)) {
    throw "No release key or signer ($secret, $signer); run tools\package-client.ps1 first."
}

if (Test-Path $Out) { Remove-Item -Recurse -Force $Out }
New-Item -ItemType Directory -Force $Out | Out-Null

$root = (Resolve-Path $Package).Path.TrimEnd('\')
$lines = @("; Gravak update manifest, made by tools\publish-update.ps1", "version=$version")
Get-ChildItem $Package -Recurse -File |
    Where-Object { $_.Name -ne "gravak.ini" -and $_.FullName -notmatch '\\settings\\' } |
    Sort-Object FullName |
    ForEach-Object {
        $relative = $_.FullName.Substring($root.Length + 1)
        $target = Join-Path $Out $relative
        New-Item -ItemType Directory -Force (Split-Path -Parent $target) | Out-Null
        Copy-Item $_.FullName $target
        $hash = (Get-FileHash -Algorithm SHA256 $_.FullName).Hash.ToLowerInvariant()
        $lines += "file=$($relative.Replace('\', '/'))|$($_.Length)|$hash"
        Write-Host ("  {0,-28} {1,12:N0} bytes" -f $relative, $_.Length)
    }
# Written last, so a client never finds a manifest naming files not yet there.
$manifest = Join-Path $Out "manifest.txt"
$lines -join "`n" | Set-Content -NoNewline -Encoding ascii $manifest
& $signer sign $secret $manifest
if ($LASTEXITCODE -ne 0) { throw "gravak-sign failed" }
Write-Host "dist\update ready for v$version. Upload its contents (manifest.sig and manifest.txt last)." -ForegroundColor Green
