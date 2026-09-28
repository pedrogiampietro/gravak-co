@echo off
setlocal
title Gravak-CO - Run Client

set "CLIENT_EXE=%~dp0client\target\release\conquer-sandbox.exe"

if not exist "%CLIENT_EXE%" (
    echo Client build not found. Building it now...
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0build-client.ps1"
    if errorlevel 1 (
        echo.
        echo Build failed; the client cannot be started.
        pause
        exit /b 1
    )
)

pushd "%~dp0client"
if /I "%~1"=="offline" goto offline

"%CLIENT_EXE%" --assets "R:\5065" --online --auth 127.0.0.1:9958 --username test1 --password 123456 --realm CoEmu
set "CLIENT_EXIT=%ERRORLEVEL%"
popd
if not "%CLIENT_EXIT%"=="0" (
    echo.
    echo Client exited with an error. Check that the local servers are running.
    pause
)
exit /b

:offline
"%CLIENT_EXE%" --assets "R:\5065"
set "CLIENT_EXIT=%ERRORLEVEL%"
popd
if not "%CLIENT_EXIT%"=="0" (
    echo.
    echo Client exited with an error.
    pause
)
