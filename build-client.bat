@echo off
title Gravak-CO - Build Client
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0build-client.ps1"
if errorlevel 1 (
    echo.
    echo Client build failed. Review the error above.
) else (
    echo.
    echo Client build completed successfully.
)
echo.
pause
