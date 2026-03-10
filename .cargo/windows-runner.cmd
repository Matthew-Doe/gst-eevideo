@echo off
setlocal
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0windows-runner.ps1" %*
exit /b %ERRORLEVEL%
