$ErrorActionPreference = 'Stop'

$projectRoot = $PSScriptRoot
$clientRoot = Join-Path $projectRoot 'client'
$editorExecutable = Join-Path $clientRoot 'target\release\gravak-editor.exe'

Push-Location $clientRoot
try {
    Write-Host 'Building Gravak Editor and gravak-content (Release)...' -ForegroundColor Cyan
    & cargo build --release --locked -p gravak-editor -p gravak-content
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build failed with exit code $LASTEXITCODE"
    }

    if (-not (Test-Path -LiteralPath $editorExecutable -PathType Leaf)) {
        throw "Build succeeded, but the editor executable was not found: $editorExecutable"
    }

    Write-Host "Build complete: $editorExecutable" -ForegroundColor Green
}
finally {
    Pop-Location
}
