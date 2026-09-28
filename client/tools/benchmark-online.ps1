param(
    [int]$DurationSeconds = 8,
    [string]$AssetsPath = 'R:\5065',
    [string]$Username = 'test1',
    [string]$Password = '123456',
    [ValidateRange(1, 64)]
    [int]$TextureLoadsPerFrame = 1,
    [int[]]$Spawn
)

$ErrorActionPreference = 'Stop'
$clientRoot = Split-Path -Parent $PSScriptRoot
$binaryPath = Join-Path $clientRoot 'target/release/conquer-sandbox.exe'
$assetPath = [IO.Path]::GetFullPath($AssetsPath)
$logRoot = Join-Path $env:TEMP 'gravak-online-bench'
$errorPath = Join-Path $logRoot 'client.err.log'
$outputPath = Join-Path $logRoot 'client.out.log'

if (-not (Test-Path -LiteralPath $binaryPath)) {
    throw "Release client not found: $binaryPath"
}
if (-not (Test-Path -LiteralPath $assetPath -PathType Container)) {
    throw "Assets directory not found: $assetPath"
}

New-Item -ItemType Directory -Force -Path $logRoot | Out-Null
Remove-Item -LiteralPath $errorPath, $outputPath -Force -ErrorAction SilentlyContinue

$saved = @{
    GRAVAK_PERF = $env:GRAVAK_PERF
    GRAVAK_PRESENT_MODE = $env:GRAVAK_PRESENT_MODE
    GRAVAK_ASSET_LOADS_PER_FRAME = $env:GRAVAK_ASSET_LOADS_PER_FRAME
}

try {
    $env:GRAVAK_PERF = '1'
    $env:GRAVAK_PRESENT_MODE = 'immediate'
    $env:GRAVAK_ASSET_LOADS_PER_FRAME = $TextureLoadsPerFrame.ToString()
    $arguments = @(
        '--assets', $assetPath,
        '--online',
        '--auth', '127.0.0.1:9958',
        '--username', $Username,
        '--password', $Password,
        '--realm', 'CoEmu'
    )
    if ($Spawn.Count -eq 2) {
        $arguments += @('--spawn', $Spawn[0], $Spawn[1])
    }
    $clientProcess = Start-Process -FilePath $binaryPath -WorkingDirectory $clientRoot `
        -ArgumentList $arguments -RedirectStandardError $errorPath `
        -RedirectStandardOutput $outputPath -PassThru
    Start-Sleep -Seconds $DurationSeconds
    if (-not $clientProcess.HasExited) {
        Stop-Process -Id $clientProcess.Id -Force
        $clientProcess.WaitForExit()
    }
    Select-String -Path $errorPath -Pattern 'PERF frames=|Authenticated|logged|map|ONLINE' |
        ForEach-Object { $_.Line }
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
