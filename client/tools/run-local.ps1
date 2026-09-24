param(
    [string]$AssetsPath = 'R:\5065',
    [switch]$CheckAssets,
    [int]$Map = 1002,
    [int]$Body = 3,
    [int[]]$Spawn
)

$ErrorActionPreference = 'Stop'

$projectRoot = Split-Path -Parent $PSScriptRoot
$assetsFullPath = [IO.Path]::GetFullPath($AssetsPath)

if (-not (Test-Path -LiteralPath $assetsFullPath -PathType Container)) {
    throw "Pasta de assets não encontrada: $assetsFullPath"
}

$cargoArgs = @('--assets', $assetsFullPath, '--map', $Map, '--body', $Body)
if ($Spawn.Count -eq 2) {
    $cargoArgs += @('--spawn', $Spawn[0], $Spawn[1])
}
if ($CheckAssets) {
    $cargoArgs += '--check-assets'
}

Push-Location $projectRoot
try {
    cargo run --release --locked -- @cargoArgs
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
} finally {
    Pop-Location
}
