@echo off
title KRIPUS LAUNCHER - GTA VI
cd /d "%~dp0"
powershell.exe -NoProfile -ExecutionPolicy Bypass -STA -File "%~dp0kripus_launcher.ps1"
if %ERRORLEVEL% NEQ 0 (
    echo.
    echo [BLAD] Wystapil blad podczas uruchamiania launchera.
    pause
)
