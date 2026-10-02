$ErrorActionPreference = 'Stop'

$projectRoot = $PSScriptRoot
$serverRoot = Join-Path $projectRoot 'server'
# The same paths run-server.bat starts.
$authExecutable = Join-Path $serverRoot 'target\debug\auth-server.exe'
$gameExecutable = Join-Path $serverRoot 'target\release\game-server.exe'

if (-not (Test-Path -LiteralPath (Join-Path $serverRoot 'Cargo.toml') -PathType Leaf)) {
    throw "Server workspace not found: $serverRoot"
}

# Windows locks a running executable, so stop the servers before relinking.
foreach ($name in 'game-server', 'auth-server') {
    $running = Get-Process -Name $name -ErrorAction SilentlyContinue
    if ($running) {
        Write-Host "Stopping $name..." -ForegroundColor Yellow
        $running | Stop-Process -Force
        $running | Wait-Process -Timeout 10 -ErrorAction SilentlyContinue
    }
}

Push-Location $serverRoot
try {
    Write-Host 'Building Gravak-CO auth server (Debug)...' -ForegroundColor Cyan
    & cargo build --locked -p auth --bin auth-server --features server
    if ($LASTEXITCODE -ne 0) {
        throw "auth-server build failed with exit code $LASTEXITCODE"
    }

    Write-Host 'Building Gravak-CO game server (Release)...' -ForegroundColor Cyan
    & cargo build --release --locked -p game --bin game-server
    if ($LASTEXITCODE -ne 0) {
        throw "game-server build failed with exit code $LASTEXITCODE"
    }

    foreach ($executable in $authExecutable, $gameExecutable) {
        if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) {
            throw "Build succeeded, but the server executable was not found: $executable"
        }
    }

    Write-Host "Build complete: $authExecutable" -ForegroundColor Green
    Write-Host "Build complete: $gameExecutable" -ForegroundColor Green
    Write-Host 'Start the servers again with run-server.bat.' -ForegroundColor Green
}
finally {
    Pop-Location
}
