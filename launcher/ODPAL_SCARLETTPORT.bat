@echo off
title ScarlettPort 50%% (fx-harvest 38e8e7c) - GTA VI PC
cd /d "e:\gta6\ScarlettPort"

echo ===================================================================
echo   ScarlettPort (ZigZagBoi1 @ 48ba23a / fx-harvest 38e8e7c)
echo   * Oswietlony otwarty swiat (Series S + VICTORY_OWN_UPSCALER=1)
echo   * Klawiatura i mysz jako pad (VICTORY_KBM=1) + D3D12 Agility 1.614
echo   * Emulacja Ray-Tracing BVH + Pipeline DirectStorage
echo ===================================================================
echo Wybierz tryb uruchomienia:
echo   [1] Series S + AutoInput + Own Upscaler [ZALECANY - oswietlony swiat]
echo   [2] Series S + FastBoot + AutoInput (pomija logo i menu do swiata)
echo   [3] Series S + ReBAR + Levels-on-Demand (eksperymentalny VRAM/FPS)
echo   [4] Tryb reczny Series S (bez AutoInput, sterowanie KBM/padem)
echo ===================================================================

:: ==== Graphics Working (confirmed functional) ==== 
:: Option 1 – AutoInput + Own Upscaler (recommended)

:: ==== Graphics Non‑Working (experimental / may crash) ==== 
:: Options 2‑4 – FastBoot, ReBAR, Manual mode

set "choice=1"
set /p "choice=Wybierz opcje [1-4] i wcisnij Enter (lub Enter dla 1): "

if "%choice%"=="2" goto fast_boot
if "%choice%"=="3" goto rebar_mode
if "%choice%"=="4" goto manual_mode

echo Uruchamianie: ScarlettPort Series S + AutoInput + Own Upscaler...
powershell -ExecutionPolicy Bypass -File "e:\gta6\ScarlettPort\scripts\play-victory.ps1" -Game "e:\gta6\GTA6.Xbox.v1.0.2.1-CYBERLEEK" -Console series-s -AutoInput
goto end

:fast_boot
echo Uruchamianie: ScarlettPort Series S + FastBoot + AutoInput...
powershell -ExecutionPolicy Bypass -File "e:\gta6\ScarlettPort\scripts\play-victory.ps1" -Game "e:\gta6\GTA6.Xbox.v1.0.2.1-CYBERLEEK" -Console series-s -AutoInput -FastBoot
goto end

:rebar_mode
echo Uruchamianie: ScarlettPort Series S + ReBAR + AutoInput...
set VICTORY_REBAR=1
powershell -ExecutionPolicy Bypass -File "e:\gta6\ScarlettPort\scripts\play-victory.ps1" -Game "e:\gta6\GTA6.Xbox.v1.0.2.1-CYBERLEEK" -Console series-s -AutoInput
goto end

:manual_mode
echo Uruchamianie: ScarlettPort Series S (sterowanie reczne KBM/Pad)...
powershell -ExecutionPolicy Bypass -File "e:\gta6\ScarlettPort\scripts\play-victory.ps1" -Game "e:\gta6\GTA6.Xbox.v1.0.2.1-CYBERLEEK" -Console series-s
goto end

:end
pause