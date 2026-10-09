@echo off
title Gravak - Release Client
rem Builds the client and publishes it to players: the update their clients
rem download on start and the installer on the website (client\tools\release-client.ps1).
rem   release-client.bat
rem   release-client.bat -Assets D:\Conquer5065
rem   release-client.bat -NoInstaller
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0client\tools\release-client.ps1" %*
if errorlevel 1 (
    echo.
    echo Release failed; see the error above.
)
pause
