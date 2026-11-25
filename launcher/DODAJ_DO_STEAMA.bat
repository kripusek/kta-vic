@echo off
title Dodawanie Grand Theft Auto VI do Steam
cd /d "e:\gta6"
echo ===================================================================
echo   Dodawanie Grand Theft Auto VI do Twojej biblioteki Steam...
echo ===================================================================
python "e:\gta6\dodaj_do_steama.py"
echo.
echo ===================================================================
echo   GOTOWE!
echo   Wskazowka: Jesli Steam byl otwarty, zamknij go i wlacz ponownie
echo   (Steam czyta shortcuts.vdf tylko podczas uruchamiania programu).
echo ===================================================================
pause
