<#
.SYNOPSIS
    Readies the accounts for players: only the team stay game masters, and
    the development accounts (password 123456) are locked.

.DESCRIPTION
    Makes a backup first (db-backup.ps1). Then:
    - every account loses game master, except the usernames in -GameMasters;
    - the development accounts (gravak, test1, and the *test accounts of seed_tests.rs) are banned
      for good, unless named in -GameMasters (change their password first).
    Run it once, before others play; it can be run again.

.EXAMPLE
    .\server\scripts\prepare-launch.ps1 -GameMasters pedro,ana
#>
param(
    [Parameter(Mandatory = $true)]
    [string[]] $GameMasters,
    [string] $Container = 'gravak-postgres'
)
$ErrorActionPreference = 'Stop'

foreach ($name in $GameMasters) {
    if ($name -notmatch '^[A-Za-z0-9_]{3,16}$') {
        throw "Nome de conta invalido: '$name'"
    }
}

& (Join-Path $PSScriptRoot 'db-backup.ps1')

$list = ($GameMasters | ForEach-Object { "'" + $_.ToLower() + "'" }) -join ','
$sql = @"
UPDATE accounts SET gm = (lower(username) IN ($list));
UPDATE accounts SET banned_until = 'infinity', ban_reason = 'development account'
 WHERE username IN ('gravak', 'test1', 'watertest', 'trojantest', 'warriortest', 'archertest') AND lower(username) NOT IN ($list);
SELECT username, gm, banned_until IS NOT NULL AS banned FROM accounts
 WHERE gm OR banned_until IS NOT NULL ORDER BY username;
"@
& docker exec $Container psql -U gravak -d gravak -v ON_ERROR_STOP=1 -c $sql
if ($LASTEXITCODE -ne 0) { throw "psql falhou (codigo $LASTEXITCODE)" }
Write-Host "Contas prontas: GMs = $($GameMasters -join ', '); contas de desenvolvimento travadas." -ForegroundColor Green
