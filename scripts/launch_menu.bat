@echo off
title VIctory / kta-vic - Launch Menu
cd /d "%~dp0\.."

echo ===================================================================
echo   kta-vic - GTA VI (Xbox Scarlett PC Compatibility Layer)
echo ===================================================================
echo Wybierz tryb uruchomienia:
echo   [1] Tryb Series X (Zoptymalizowany: GPU timing + limit petli)
echo   [2] Tryb Series S (Nizszy narzut GPU, wyzsza wydajnosc FPS)
echo   [3] Tryb programowy WARP (Renderowanie CPU)
echo ===================================================================
set /p choice="Wybierz opcje (1/2/3, domyslnie 1): "

set VICTORY_GPU_TIMING=1
set VICTORY_SHADER_LOOP_LIMIT=128

if "%choice%"=="2" goto series_s
if "%choice%"=="3" goto warp
goto series_x

:series_x
echo Uruchamianie w trybie Xbox Series X...
target\release\victory.exe run "%VICTORY_GAME_DIR%\game_scarlett_master.exe" --execute --unimplemented zero --console series-x
goto end

:series_s
echo Uruchamianie w trybie Xbox Series S...
target\release\victory.exe run "%VICTORY_GAME_DIR%\game_scarlett_master.exe" --execute --unimplemented zero --console series-s
goto end

:warp
echo Uruchamianie w trybie programowym WARP...
set VICTORY_D3D12_WARP=1
target\release\victory.exe run "%VICTORY_GAME_DIR%\game_scarlett_master.exe" --execute --unimplemented zero
goto end

:end
pause
