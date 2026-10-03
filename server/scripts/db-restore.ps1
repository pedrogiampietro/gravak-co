<#
.SYNOPSIS
    Puts a backup made by db-backup.ps1 back into the Postgres database.

.DESCRIPTION
    Replaces everything in the database with the backup. The servers must be
    stopped (run-server.bat stop). A backup of the current state is made first,
    so a restore can itself be undone.

.EXAMPLE
    .\server\scripts\db-restore.ps1 server\backups\gravak-2026-10-03-092500.dump
#>
param(
    [Parameter(Mandatory = $true)] [string] $Backup,
    [string] $Container = 'gravak-postgres',
    [switch] $Yes
)
$ErrorActionPreference = 'Stop'

$file = Resolve-Path $Backup
if (Get-Process -Name game-server, auth-server -ErrorAction SilentlyContinue) {
    throw 'Pare os servidores antes (run-server.bat stop).'
}
if (-not $Yes) {
    $answer = Read-Host "Substituir TODO o banco pelo backup $(Split-Path $file -Leaf)? Digite sim"
    if ($answer -ne 'sim') { Write-Host 'Cancelado.'; exit 1 }
}

Write-Host 'Salvando o estado atual antes de restaurar...' -ForegroundColor Cyan
& (Join-Path $PSScriptRoot 'db-backup.ps1') -Container $Container

$inside = '/tmp/restore.dump'
& docker cp $file "${Container}:$inside"
if ($LASTEXITCODE -ne 0) { throw "docker cp falhou (codigo $LASTEXITCODE)" }
& docker exec $Container pg_restore -U gravak -d gravak --clean --if-exists --no-owner --single-transaction $inside
$restored = $LASTEXITCODE
& docker exec $Container rm -f $inside | Out-Null
if ($restored -ne 0) { throw "pg_restore falhou (codigo $restored); o banco ficou como estava." }
Write-Host "Backup restaurado: $(Split-Path $file -Leaf)" -ForegroundColor Green
