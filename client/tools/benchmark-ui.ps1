param(
    [string[]]$Screens = @('character', 'inventory', 'shop', 'warehouse', 'guild'),
    [int]$DurationSeconds = 6,
    [string]$AssetsPath = 'R:\5065'
)

$ErrorActionPreference = 'Stop'

$clientRoot = Split-Path -Parent $PSScriptRoot
$binaryPath = Join-Path $clientRoot 'target/release/conquer-sandbox.exe'
$assetPath = [IO.Path]::GetFullPath($AssetsPath)
$logRoot = Join-Path $env:TEMP 'gravak-ui-bench'

if (-not (Test-Path -LiteralPath $binaryPath)) {
    throw "Release client not found: $binaryPath"
}
if (-not (Test-Path -LiteralPath $assetPath -PathType Container)) {
    throw "Assets directory not found: $assetPath"
}

New-Item -ItemType Directory -Force -Path $logRoot | Out-Null
$saved = @{
    GRAVAK_PERF = $env:GRAVAK_PERF
    GRAVAK_PRESENT_MODE = $env:GRAVAK_PRESENT_MODE
    GRAVAK_ASSET_LOADS_PER_FRAME = $env:GRAVAK_ASSET_LOADS_PER_FRAME
    GRAVAK_BENCHMARK_SCREEN = $env:GRAVAK_BENCHMARK_SCREEN
}

try {
    $env:GRAVAK_PERF = '1'
    $env:GRAVAK_PRESENT_MODE = 'immediate'
    $env:GRAVAK_ASSET_LOADS_PER_FRAME = '1'

    foreach ($screenName in $Screens) {
        $errorPath = Join-Path $logRoot "$screenName.err.log"
        $outputPath = Join-Path $logRoot "$screenName.out.log"
        Remove-Item -LiteralPath $errorPath, $outputPath -Force -ErrorAction SilentlyContinue
        $env:GRAVAK_BENCHMARK_SCREEN = $screenName

        $screenProcess = Start-Process -FilePath $binaryPath -WorkingDirectory $clientRoot `
            -ArgumentList @('--assets', $assetPath, '--map', '1002', '--body', '3') `
            -RedirectStandardError $errorPath -RedirectStandardOutput $outputPath -PassThru
        Start-Sleep -Seconds $DurationSeconds
        if (-not $screenProcess.HasExited) {
            Stop-Process -Id $screenProcess.Id -Force
            $screenProcess.WaitForExit()
        }

        Write-Output "[$screenName]"
        $perfLines = Select-String -Path $errorPath -Pattern 'PERF frames=' -ErrorAction SilentlyContinue
        if ($perfLines) {
            $perfLines | ForEach-Object { $_.Line }
        } else {
            Write-Output 'no perf line'
        }
    }
}
finally {
    foreach ($variableName in $saved.Keys) {
        $savedValue = $saved[$variableName]
        if ($null -eq $savedValue) {
            Remove-Item "Env:$variableName" -ErrorAction SilentlyContinue
        } else {
            Set-Item "Env:$variableName" $savedValue
        }
    }
}
