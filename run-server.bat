@echo off
setlocal
title Gravak-CO - Server

echo.
echo    _____                     _
echo   / ____^|                   ^| ^|
echo  ^| ^|  __ _ __ __ ___   ____ _^| ^| __
echo  ^| ^| ^|_ ^| '__/ _` \ \ / / _` ^| ^|/ /
echo  ^| ^|__^| ^| ^| ^| (_^| ^|\ V / (_^| ^|   ^<
echo   \_____^|_^|  \__,_^| \_/ \__,_^|_^|\_\
echo.
echo   Gravak-CO Server Launcher
echo   =========================
echo.

set "SERVER_DIR=%~dp0server"
set "AUTH_EXE=%SERVER_DIR%\target\debug\auth-server.exe"
set "GAME_EXE=%SERVER_DIR%\target\release\game-server.exe"

rem -- Build -------------------------------------------------------
rem Rebuild before every start (not for stop/status) so a git pull takes
rem effect; cargo only recompiles what changed, and the build script stops
rem running servers before relinking.
if /I not "%~1"=="stop" if /I not "%~1"=="status" (
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0build-server.ps1"
    if errorlevel 1 (
        echo.
        echo [ERRO] Falha ao compilar os servidores; veja o erro acima.
        pause
        exit /b 1
    )
    echo.
)

rem -- Content -----------------------------------------------------
rem content\ (items, icons, map objects) built over the installation: the
rem server takes its itemtype.dat over the one named in server\.env and
rem closes the cells our map objects stand on. Not for stop/status.
if /I not "%~1"=="stop" if /I not "%~1"=="status" (
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0build-content.ps1" || (
        echo [AVISO] content\ nao foi gerado; os servidores sobem sem ele.
    )
)
if exist "%~dp0content\build\client\ini\itemtype.dat" set "ITEMTYPE_DATA=%~dp0content\build\client\ini\itemtype.dat"
if exist "%~dp0content\build\client\.gravak-build" set "GRAVAK_CONTENT=%~dp0content\build\client"

rem -- Check binaries ----------------------------------------------
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

rem -- Parse arguments ---------------------------------------------
if /I "%~1"=="stop" goto stop
if /I "%~1"=="status" goto status

rem -- Database ----------------------------------------------------
rem The servers keep their data in the Postgres container of
rem server\docker-compose.yml; start it (and Docker Desktop) if needed.
docker info >nul 2>&1 || (
    echo Iniciando o Docker Desktop...
    start "" "%ProgramFiles%\Docker\Docker\Docker Desktop.exe"
    powershell.exe -NoProfile -Command "for ($i = 0; $i -lt 90; $i++) { docker info *> $null; if ($LASTEXITCODE -eq 0) { exit 0 }; Start-Sleep 2 }; exit 1"
)
docker info >nul 2>&1 || (
    echo [ERRO] O Docker nao respondeu; abra o Docker Desktop e tente de novo.
    pause
    exit /b 1
)
echo Iniciando o Postgres...
docker compose -f "%SERVER_DIR%\docker-compose.yml" up -d --wait >nul
if errorlevel 1 (
    echo [ERRO] O Postgres nao subiu; veja: docker compose -f server\docker-compose.yml logs
    pause
    exit /b 1
)
rem One backup a day into server\backups (the newest 14 are kept).
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%SERVER_DIR%\scripts\db-backup.ps1" -Daily || (
    echo [AVISO] O backup do banco falhou; os servidores sobem mesmo assim.
)

rem -- Start servers -----------------------------------------------
rem A server left over from an earlier run holds its port, and the new one
rem would exit with "address in use"; stop the old ones first.
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%SERVER_DIR%\scripts\stop-servers.ps1" >nul
taskkill /FI "WINDOWTITLE eq Gravak - Auth Server*" /F >nul 2>&1
taskkill /FI "WINDOWTITLE eq Gravak - Game Server*" /F >nul 2>&1

echo [1/2] Iniciando Auth Server...
pushd "%SERVER_DIR%"
rem cmd /k keeps the window open if the server exits, so its error stays visible.
start "Gravak - Auth Server" /MIN cmd /k ""%AUTH_EXE%""
popd

rem Small delay to let the auth server bind its port
timeout /t 2 /nobreak >nul

echo [2/2] Iniciando Game Server...
pushd "%SERVER_DIR%"
start "Gravak - Game Server" /MIN cmd /k ""%GAME_EXE%""
popd

rem The game server loads the database and maps before binding its port.
timeout /t 5 /nobreak >nul

set "FAILED="
tasklist /FI "IMAGENAME eq auth-server.exe" 2>nul | find /I "auth-server.exe" >nul || (
    echo [ERRO] O Auth Server encerrou logo apos iniciar.
    echo        Veja o erro na janela "Gravak - Auth Server".
    set "FAILED=1"
)
tasklist /FI "IMAGENAME eq game-server.exe" 2>nul | find /I "game-server.exe" >nul || (
    echo [ERRO] O Game Server encerrou logo apos iniciar.
    echo        Veja o erro na janela "Gravak - Game Server".
    set "FAILED=1"
)
if defined FAILED (
    echo.
    pause
    exit /b 1
)

echo.
echo   +-----------------------------------------+
echo   ^|  Auth Server  -  porta 9958  (login)    ^|
echo   ^|  Game Server  -  porta 5816  (jogo)     ^|
echo   +-----------------------------------------+
echo.
echo   Ambos os servidores foram iniciados em janelas
echo   minimizadas. Use  run-server.bat stop  para
echo   encerrar, ou  run-server.bat status  para
echo   verificar se estao rodando.
echo.
echo   Para conectar o cliente:  run-client.bat
echo.
goto :eof

rem -- Stop servers ------------------------------------------------
:stop
echo Encerrando servidores (o game salva quem esta online)...
rem Ctrl+Break in each server's window, as if it were closed; the game
rem server saves everyone online before it exits.
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%SERVER_DIR%\scripts\stop-servers.ps1"
rem Close the console windows that hosted the servers.
taskkill /FI "WINDOWTITLE eq Gravak - Auth Server*" /F >nul 2>&1
taskkill /FI "WINDOWTITLE eq Gravak - Game Server*" /F >nul 2>&1
echo.
echo Servidores encerrados.
goto :eof

rem -- Status check ------------------------------------------------
:status
echo Verificando servidores...
echo.
tasklist /FI "IMAGENAME eq auth-server.exe" 2>nul | find /I "auth-server.exe" >nul && (
    echo   [OK] Auth Server  -  RODANDO
) || (
    echo   [XX] Auth Server  -  PARADO
)
tasklist /FI "IMAGENAME eq game-server.exe" 2>nul | find /I "game-server.exe" >nul && (
    echo   [OK] Game Server  -  RODANDO
) || (
    echo   [XX] Game Server  -  PARADO
)
echo.
goto :eof
