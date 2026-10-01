@echo off
setlocal
title Gravak-CO - Server

echo.
echo   _____         _____
echo  /  __ \       ^|  ___^|
echo  ^| /  \/  ___  ^| ^|__  _ __ ___   _   _
echo  ^| ^|     / _ \ ^|  __^|^| '_ ` _ \ ^| ^| ^| ^|
echo  ^| \__/\^| (_) ^|^| ^|___^| ^| ^| ^| ^| ^|^| ^|_^| ^|
echo   \____/ \___/ \____/^|_^| ^|_^| ^|_^| \__,_^|
echo.
echo   Gravak-CO Server Launcher
echo   =========================
echo.

set "SERVER_DIR=%~dp0server"
set "AUTH_EXE=%SERVER_DIR%\target\debug\auth-server.exe"
set "GAME_EXE=%SERVER_DIR%\target\release\game-server.exe"

rem ── Check binaries ──────────────────────────────────────────────
if not exist "%AUTH_EXE%" (
    echo [ERRO] Auth server nao encontrado em:
    echo        %AUTH_EXE%
    echo.
    echo        Compile primeiro com: cargo build -p auth-server
    pause
    exit /b 1
)

if not exist "%GAME_EXE%" (
    echo [ERRO] Game server nao encontrado em:
    echo        %GAME_EXE%
    echo.
    echo        Compile primeiro com: cargo build --release -p game-server
    pause
    exit /b 1
)

rem ── Parse arguments ─────────────────────────────────────────────
if /I "%~1"=="stop" goto stop
if /I "%~1"=="status" goto status

rem ── Start servers ───────────────────────────────────────────────
echo [1/2] Iniciando Auth Server...
pushd "%SERVER_DIR%"
start "CoEmu - Auth Server" /MIN "%AUTH_EXE%"
popd

rem Small delay to let the auth server bind its port
timeout /t 2 /nobreak >nul

echo [2/2] Iniciando Game Server...
pushd "%SERVER_DIR%"
start "CoEmu - Game Server" /MIN "%GAME_EXE%"
popd

timeout /t 2 /nobreak >nul

echo.
echo   ┌─────────────────────────────────────────┐
echo   │  Auth Server  ─  porta 9958  (login)    │
echo   │  Game Server  ─  porta 5816  (jogo)     │
echo   └─────────────────────────────────────────┘
echo.
echo   Ambos os servidores foram iniciados em janelas
echo   minimizadas. Use  run-server.bat stop  para
echo   encerrar, ou  run-server.bat status  para
echo   verificar se estao rodando.
echo.
echo   Para conectar o cliente:  run-client.bat
echo.
goto :eof

rem ── Stop servers ────────────────────────────────────────────────
:stop
echo Encerrando servidores...
taskkill /IM auth-server.exe /F >nul 2>&1 && (
    echo   [OK] Auth Server encerrado.
) || (
    echo   [--] Auth Server nao estava rodando.
)
taskkill /IM game-server.exe /F >nul 2>&1 && (
    echo   [OK] Game Server encerrado.
) || (
    echo   [--] Game Server nao estava rodando.
)
echo.
echo Servidores encerrados.
goto :eof

rem ── Status check ────────────────────────────────────────────────
:status
echo Verificando servidores...
echo.
tasklist /FI "IMAGENAME eq auth-server.exe" 2>nul | find /I "auth-server.exe" >nul && (
    echo   [OK] Auth Server  ─  RODANDO
) || (
    echo   [XX] Auth Server  ─  PARADO
)
tasklist /FI "IMAGENAME eq game-server.exe" 2>nul | find /I "game-server.exe" >nul && (
    echo   [OK] Game Server  ─  RODANDO
) || (
    echo   [XX] Game Server  ─  PARADO
)
echo.
goto :eof
