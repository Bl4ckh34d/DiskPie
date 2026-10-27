@echo off
if exist "%~dp0dist\DiskPie\DiskPie.exe" (
  start "" "%~dp0dist\DiskPie\DiskPie.exe" %*
) else (
  echo Build the portable app first: powershell -ExecutionPolicy Bypass -File scripts/package-windows.ps1
  pause
)
