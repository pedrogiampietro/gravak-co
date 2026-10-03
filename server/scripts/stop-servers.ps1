<#
.SYNOPSIS
    Stops the auth and game servers the way closing their windows does.

.DESCRIPTION
    The game server saves every connected character when asked to stop; a
    plain kill loses what changed since the last autosave. Each server gets
    Ctrl+Break in its own console (Ctrl+C is ignored by programs started with
    `start`) and some seconds to finish; one still running after -Timeout
    seconds is killed.
#>
param([int] $Timeout = 20)

$signature = @'
using System;
using System.Runtime.InteropServices;
public static class ConsoleSignal {
    [DllImport("kernel32.dll", SetLastError = true)] public static extern bool AttachConsole(uint processId);
    [DllImport("kernel32.dll", SetLastError = true)] public static extern bool FreeConsole();
    [DllImport("kernel32.dll", SetLastError = true)] public static extern bool SetConsoleCtrlHandler(IntPtr handler, bool add);
    [DllImport("kernel32.dll", SetLastError = true)] public static extern bool GenerateConsoleCtrlEvent(uint ctrlEvent, uint processGroupId);
}
'@

foreach ($name in 'game-server', 'auth-server') {
    foreach ($process in @(Get-Process -Name $name -ErrorAction SilentlyContinue)) {
        # A process can only signal the console it is attached to, so a child
        # PowerShell leaves its own console, joins the server's and sends
        # Ctrl+Break there.
        $script = @"
Add-Type -TypeDefinition @'
$signature
'@
[ConsoleSignal]::FreeConsole() | Out-Null
if ([ConsoleSignal]::AttachConsole($($process.Id))) {
    [ConsoleSignal]::GenerateConsoleCtrlEvent(1, 0) | Out-Null
    Start-Sleep -Milliseconds 500
    [ConsoleSignal]::FreeConsole() | Out-Null
}
"@
        $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($script))
        Start-Process powershell.exe -ArgumentList '-NoProfile', '-EncodedCommand', $encoded -WindowStyle Hidden -Wait
        if ($process.WaitForExit($Timeout * 1000)) {
            Write-Host "  [OK] $name encerrado normalmente."
        } else {
            $process | Stop-Process -Force
            Write-Host "  [!!] $name nao respondeu em $Timeout s; encerrado a forca."
        }
    }
}
