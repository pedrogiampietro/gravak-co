@echo off
rem Builds the client and packs it for players: client\dist\Gravak, and
rem client\dist\GravakSetup.exe when Inno Setup is installed.
rem   package-client.bat
rem   package-client.bat -Assets D:\Conquer5065 -Auth 192.168.0.10:9958
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0client\tools\package-client.ps1" %*
if errorlevel 1 (
    echo.
    echo Packaging failed; see the error above.
)
pause
