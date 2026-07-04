@echo off
cd /d C:\GitHub\Zentra\deploy
powershell -NoProfile -ExecutionPolicy Bypass -File pull-from-wsl.ps1 > C:\GitHub\Zentra\deploy\pull-output.log 2>&1
echo Exit Code: %ERRORLEVEL% >> C:\GitHub\Zentra\deploy\pull-output.log
echo.
type C:\GitHub\Zentra\deploy\pull-output.log
