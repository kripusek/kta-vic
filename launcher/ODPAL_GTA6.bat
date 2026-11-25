@echo off
title VI Causeway - GTA VI PC Compatibility Loader
cd /d "e:\gta6\victory"

echo ===================================================================
echo   VI Causeway / VIctory - Grand Theft Auto VI (PC Compatibility)
echo   Edycja ze sprzetowym kompozytorem D3D12 PresentX i optymalizacja FPS
echo ===================================================================
echo   * Multi-plane PresentX: "VICE CITY, TWO YEARS AGO" i plansze tytulowe
echo   * Asynchroniczne kolejkowanie GPU (brak zaciecia 1883 ms)
echo   * Aktywne buforowanie CLZERO i ochrona alokatora pamieci
echo ===================================================================
echo Wybierz tryb uruchomienia:
echo   [1] Series S @ 1080p 60 FPS (ZALECANY - plynny klatkaz, niskie zuzycie VRAM)
echo   [2] Series X @ 1080p 60 FPS (Wysoka jakosc swiata + skalowanie 1080p)
echo   [3] Series X @ Natywne 4K 60 FPS (Pelna jakosc 4K)
echo   [4] Series S @ UNLOCKED FPS (Maksymalna liczba klatek bez limitu)
echo   [5] Tryb Diagnostyczny (Target \diag\victory.exe ze szczegolowym logiem)
echo   [6] Uruchom przez Steam (Status w Steam: "W grze: Grand Theft Auto VI")
echo ===================================================================
set "choice=1"
set /p "choice=Wybierz opcje [1-6] i wcisnij Enter (lub po prostu wcisnij Enter dla 1): "

set VICTORY_BATCH_CLZERO=1
set VICTORY_GPU_TIMING=1
set VICTORY_D3D12_DRED=1

if "%choice%"=="6" goto steam_mode
if "%choice%"=="5" goto diag_mode
if "%choice%"=="4" goto series_s_unlocked
if "%choice%"=="3" goto series_x_4k
if "%choice%"=="2" goto series_x_1080
goto series_s_1080

:series_s_1080
echo Uruchamianie: Series S @ 1080p 60 FPS (Pelna plynnosc GPU)...
.\target\release\victory.exe run "e:\gta6\GTA6.Xbox.v1.0.2.1-CYBERLEEK\game_scarlett_master.exe" --unimplemented zero --console series-s --fps 60 --output-height 1080
goto end

:series_x_1080
echo Uruchamianie: Series X @ 1080p 60 FPS...
.\target\release\victory.exe run "e:\gta6\GTA6.Xbox.v1.0.2.1-CYBERLEEK\game_scarlett_master.exe" --unimplemented zero --console series-x --fps 60 --output-height 1080
goto end

:series_x_4k
echo Uruchamianie: Series X @ Natywne 4K 60 FPS...
.\target\release\victory.exe run "e:\gta6\GTA6.Xbox.v1.0.2.1-CYBERLEEK\game_scarlett_master.exe" --unimplemented zero --console series-x --fps 60
goto end

:series_s_unlocked
echo Uruchamianie: Series S @ UNLOCKED FPS...
set VICTORY_UNLOCK_FPS=1
.\target\release\victory.exe run "e:\gta6\GTA6.Xbox.v1.0.2.1-CYBERLEEK\game_scarlett_master.exe" --unimplemented zero --console series-s --output-height 1080
goto end

:diag_mode
echo Uruchamianie: Tryb Diagnostyczny (D3D12 Debug + GPU Timing + DRED)...
set VICTORY_D3D12_DEBUG=1
set VICTORY_D3D12_DRED=1
set VICTORY_GPU_TIMING=1
set VICTORY_LOG=victory_gpu=warn,error
.\target\diag\victory.exe run "e:\gta6\GTA6.Xbox.v1.0.2.1-CYBERLEEK\game_scarlett_master.exe" --unimplemented zero --console series-s --fps 60 --output-height 1080
goto end

:steam_mode
echo Uruchamianie przez Steam...
start steam://rungameid/2282323558
goto end

:end
pause
