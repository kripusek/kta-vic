@echo off
title kta-vic - Series S Mode (High FPS)
cd /d "%~dp0\.."
set VICTORY_GPU_TIMING=1
set VICTORY_SHADER_LOOP_LIMIT=128
target\release\victory.exe run "%VICTORY_GAME_DIR%\game_scarlett_master.exe" --execute --unimplemented zero --console series-s
pause
