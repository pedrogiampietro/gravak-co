# Builds content/ (our items, icons and map objects) over the 5065
# installation into content/build, which run-client.bat and run-server.bat
# hand to the client and the server. Nothing is rebuilt when nothing changed.
param(
    [string]$Assets = $(if ($env:GRAVAK_ASSETS) { $env:GRAVAK_ASSETS } else { 'R:\5065' })
)
$ErrorActionPreference = 'Stop'

$projectRoot = $PSScriptRoot
$clientRoot = Join-Path $projectRoot 'client'
$contentRoot = Join-Path $projectRoot 'content'
$tool = Join-Path $clientRoot 'target\release\gravak-content.exe'

Push-Location $clientRoot
try {
    & cargo build --release --locked -q -p gravak-content
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build failed with exit code $LASTEXITCODE"
    }
}
finally {
    Pop-Location
}

& $tool --assets $Assets --content $contentRoot build --if-changed
if ($LASTEXITCODE -ne 0) {
    throw "gravak-content build failed with exit code $LASTEXITCODE"
}
