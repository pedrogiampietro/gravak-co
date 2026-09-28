$ErrorActionPreference = 'Stop'

$projectRoot = $PSScriptRoot
$clientRoot = Join-Path $projectRoot 'client'
$clientExecutable = Join-Path $clientRoot 'target\release\conquer-sandbox.exe'

if (-not (Test-Path -LiteralPath (Join-Path $clientRoot 'Cargo.toml') -PathType Leaf)) {
    throw "Client workspace not found: $clientRoot"
}

Push-Location $clientRoot
try {
    Write-Host 'Building Gravak-CO client (Release)...' -ForegroundColor Cyan
    & cargo build --release --locked -p conquer-sandbox
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build failed with exit code $LASTEXITCODE"
    }

    if (-not (Test-Path -LiteralPath $clientExecutable -PathType Leaf)) {
        throw "Build succeeded, but the client executable was not found: $clientExecutable"
    }

    Write-Host "Build complete: $clientExecutable" -ForegroundColor Green
}
finally {
    Pop-Location
}
