param(
    [int]$DurationSeconds = 8,
    [string]$AssetsPath = 'R:\5065',
    [switch]$Online,
    [string]$Username = 'test1',
    [string]$Password = '123456',
    [int]$Map = 1002,
    [int]$Body = 3,
    [int[]]$Spawn,
    [int]$TextureLoadsPerFrame = 1,
    [ValidateSet(0, 60, 100, 200, 300, 400, 500)]
    [int]$MaxFps = 0
)

$ErrorActionPreference = 'Stop'
$clientRoot = Split-Path -Parent $PSScriptRoot
$binaryPath = Join-Path $clientRoot 'target/release/conquer-sandbox.exe'
$assetPath = [IO.Path]::GetFullPath($AssetsPath)
$logRoot = Join-Path $env:TEMP 'gravak-gameplay-bench'
$errorPath = Join-Path $logRoot 'client.err.log'
$outputPath = Join-Path $logRoot 'client.out.log'

if (-not (Test-Path -LiteralPath $binaryPath -PathType Leaf)) {
    throw "Release client not found: $binaryPath"
}
if (-not (Test-Path -LiteralPath $assetPath -PathType Container)) {
    throw "Assets directory not found: $assetPath"
}
if ($DurationSeconds -lt 1) {
    throw 'DurationSeconds must be at least 1.'
}

New-Item -ItemType Directory -Force -Path $logRoot | Out-Null
Remove-Item -LiteralPath $errorPath, $outputPath -Force -ErrorAction SilentlyContinue

$saved = @{
    GRAVAK_PERF = $env:GRAVAK_PERF
    GRAVAK_PRESENT_MODE = $env:GRAVAK_PRESENT_MODE
    GRAVAK_ASSET_LOADS_PER_FRAME = $env:GRAVAK_ASSET_LOADS_PER_FRAME
    GRAVAK_MAX_FPS = $env:GRAVAK_MAX_FPS
}

try {
    $env:GRAVAK_PERF = '1'
    $env:GRAVAK_PRESENT_MODE = 'immediate'
    $env:GRAVAK_ASSET_LOADS_PER_FRAME = $TextureLoadsPerFrame.ToString()
    $env:GRAVAK_MAX_FPS = $MaxFps.ToString()

    $arguments = @('--assets', $assetPath, '--map', $Map, '--body', $Body)
    if ($Online) {
        $arguments += @(
            '--online',
            '--auth', '127.0.0.1:9958',
            '--username', $Username,
            '--password', $Password,
            '--realm', 'CoEmu'
        )
    }
    if ($Spawn.Count -eq 2) {
        $arguments += @('--spawn', $Spawn[0], $Spawn[1])
    }

    $clientProcess = Start-Process -FilePath $binaryPath -WorkingDirectory $clientRoot `
        -ArgumentList $arguments -RedirectStandardError $errorPath `
        -RedirectStandardOutput $outputPath -PassThru
    try {
        $deadline = [DateTime]::UtcNow.AddSeconds($DurationSeconds)
        while (-not $clientProcess.HasExited -and [DateTime]::UtcNow -lt $deadline) {
            Start-Sleep -Milliseconds 100
        }
    }
    finally {
        if (-not $clientProcess.HasExited) {
            Stop-Process -Id $clientProcess.Id -Force
            $clientProcess.WaitForExit()
        }
    }

    Get-Content $errorPath -ErrorAction SilentlyContinue |
        Where-Object { $_ -match 'PERF frames=|FRAME_LIMIT|Authenticated|auth rejected|Requested spawn|ONLINE|online:' }
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
