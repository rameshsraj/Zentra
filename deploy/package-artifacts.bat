@echo off
REM Simple wrapper to run WSL package script
setlocal
cd /d C:\GitHub\Zentra

echo Packaging artifacts from WSL...
wsl -d Ubuntu -u root -- bash -c "cd /tmp && bash package-from-wsl.sh 2>&1 | tee /mnt/c/temp/package.log"

echo.
echo Checking output...
if exist C:\temp\package.log (
    type C:\temp\package.log
) else (
    echo No log file found
)

echo.
echo Checking artifacts directory...
dir C:\GitHub\Zentra\artifacts\vm-build\

pause
