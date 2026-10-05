<#
.SYNOPSIS
    Saves the Postgres database of docker-compose.yml to server\backups.

.DESCRIPTION
    Writes server\backups\gravak-<date>-<time>.dump (pg_dump custom format) and
    keeps the newest -Keep backups. With -Daily it does nothing when a backup
    from today already exists; run-server.bat starts the servers that way.

.EXAMPLE
    .\server\scripts\db-backup.ps1
.EXAMPLE
    .\server\scripts\db-backup.ps1 -Daily -Keep 14
#>
param(
    [switch] $Daily,
    [int] $Keep = 14,
    [string] $Container = 'gravak-postgres'
)
$ErrorActionPreference = 'Stop'

$backups = Join-Path (Split-Path $PSScriptRoot -Parent) 'backups'
New-Item -ItemType Directory -Force $backups | Out-Null

$today = Get-Date -Format 'yyyy-MM-dd'
if ($Daily -and (Get-ChildItem $backups -Filter "gravak-$today-*.dump" -ErrorAction SilentlyContinue)) {
    Write-Host "Backup de hoje ja existe em $backups" -ForegroundColor DarkGray
    exit 0
}

$name = "gravak-$today-$(Get-Date -Format 'HHmmss').dump"
$inside = "/tmp/$name"
# The dump is made inside the container and copied out: piping binary data
# through PowerShell would corrupt it.
& docker exec $Container pg_dump -U gravak -d gravak --format=custom --file=$inside
if ($LASTEXITCODE -ne 0) { throw "pg_dump falhou (codigo $LASTEXITCODE)" }
& docker cp "${Container}:$inside" (Join-Path $backups $name)
$copied = $LASTEXITCODE
& docker exec $Container rm -f $inside | Out-Null
if ($copied -ne 0) { throw "docker cp falhou (codigo $copied)" }

$size = (Get-Item (Join-Path $backups $name)).Length / 1KB
Write-Host ("Backup salvo: {0} ({1:N0} KB)" -f (Join-Path $backups $name), $size) -ForegroundColor Green

Get-ChildItem $backups -Filter 'gravak-*.dump' |
    Sort-Object Name -Descending |
    Select-Object -Skip $Keep |
    ForEach-Object {
        Remove-Item $_.FullName
        Write-Host "Backup antigo removido: $($_.Name)" -ForegroundColor DarkGray
    }
