@echo off
setlocal
title Gravak-CO - Run Client

set "CLIENT_EXE=%~dp0client\target\release\conquer-sandbox.exe"

rem Rebuild before every launch so a git pull takes effect; cargo only
rem recompiles what changed. A running client locks the exe, so skip the
rem build then and start the existing one.
tasklist /FI "IMAGENAME eq conquer-sandbox.exe" 2>nul | find /I "conquer-sandbox.exe" >nul && (
    echo A client is already running; skipping the rebuild.
) || (
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0build-client.ps1"
    if errorlevel 1 (
        echo.
        echo Build failed; the client cannot be started.
        pause
        exit /b 1
    )
)

rem Our content (items, icons, map objects) built over the installation;
rem the client reads it before R:\5065. A failed build leaves the game
rem without it, but still starts.
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0build-content.ps1" || (
    echo [WARNING] content\ could not be built; the client starts without it.
)
if exist "%~dp0content\build\client\.gravak-build" set "GRAVAK_CONTENT=%~dp0content\build\client"

if not exist "%CLIENT_EXE%" (
    echo Client build not found: %CLIENT_EXE%
    pause
    exit /b 1
)

pushd "%~dp0client"
if /I "%~1"=="offline" goto offline
if /I "%~1"=="login" goto login

rem One account per client: the same account cannot be online twice.
rem   run-client.bat            opens two clients (test1 and gravak)
rem   run-client.bat <account>  opens one client with that account
rem   run-client.bat login      opens the login screens (register/create)
rem run-client.bat <account> [password]: one client, signed in; the
rem password is the development accounts' 123456 when not given.
if not "%~1"=="" (
    call :client "%~1" "%~2"
    popd
    exit /b
)
call :client test1
rem Let the first login settle before the second.
timeout /t 2 /nobreak >nul
call :client gravak
popd
exit /b

:client
rem cmd /c ... || pause keeps the window open when the client exits with an
rem error (for example, the game server is not running), so it can be read.
set "CLIENT_PASSWORD=%~2"
if "%CLIENT_PASSWORD%"=="" set "CLIENT_PASSWORD=123456"
start "Gravak-CO - %~1" cmd /c ""%CLIENT_EXE%" --assets "R:\5065" --online --auth 127.0.0.1:9958 --username %~1 --password %CLIENT_PASSWORD% --realm Gravak || pause"
exit /b

:login
start "Gravak-CO - Login" cmd /c ""%CLIENT_EXE%" --assets "R:\5065" --online --auth 127.0.0.1:9958 --realm Gravak || pause"
popd
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
