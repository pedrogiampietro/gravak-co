@echo off
title Gravak-CO - Build Server
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0build-server.ps1"
if errorlevel 1 (
    echo.
    echo Server build failed. Review the error above.
) else (
    echo.
    echo Server build completed successfully.
)
echo.
pause
