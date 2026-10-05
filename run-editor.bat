@echo off
setlocal
title Gravak-CO - Editor

set "EDITOR_EXE=%~dp0client\target\release\gravak-editor.exe"

rem Rebuild before every launch so a git pull takes effect; cargo only
rem recompiles what changed. A running editor locks the exe, so skip the
rem build then and start the existing one.
tasklist /FI "IMAGENAME eq gravak-editor.exe" 2>nul | find /I "gravak-editor.exe" >nul && (
    echo The editor is already running; skipping the rebuild.
) || (
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0build-editor.ps1"
    if errorlevel 1 (
        echo.
        echo Build failed; the editor cannot be started.
        pause
        exit /b 1
    )
)

rem The 5065 installation the editor reads; the first time it opens with
rem this one, afterwards with the one chosen in the editor.
if not defined GRAVAK_ASSETS set "GRAVAK_ASSETS=R:\5065"

rem Started from the repository, the editor finds its content folder.
pushd "%~dp0"
start "" "%EDITOR_EXE%"
popd
