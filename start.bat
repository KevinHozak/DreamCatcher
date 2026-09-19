@echo off
title DreamCatcher - Local AI Photo Triage Cockpit
color 0b

echo ========================================================
echo   __  __                             _         _               
echo  ^|  \/  ^| ___   ___  _ __   __| ^| _ __ ___  __ _ _ __ ___  
echo  ^| ^|\/^| ^|/ _ \ / _ \^| '_ \ / _` ^|^| '__/ _ \/ _` ^| '_ ` _ \ 
echo  ^| ^|  ^| ^| (_) ^| (_) ^| ^| ^| ^| (_^| ^|^| ^| ^|  __/ (_^| ^| ^| ^| ^| ^| ^|
echo  ^|_^|  ^|_^|\___/ \___/^|_^| ^|_^|\__,_^|^|_^|  \___^|\__,_^|_^| ^|_^| ^|_^|
echo                    D R E A M C A T C H E R
echo ========================================================
echo.

cd /d "%~dp0"

:: 1. Check Ollama
echo [1/2] Checking local Ollama service...
curl -s http://127.0.0.1:11434/api/tags >nul 2>&1
if %errorlevel% neq 0 (
    echo     Starting Ollama in background...
    start /B "" "%LOCALAPPDATA%\Programs\Ollama\ollama.exe" serve
    timeout /t 2 /nobreak >nul
) else (
    echo     Ollama is running.
)

:: 2. Launch Standalone Native Desktop App
echo [2/2] Launching DreamCatcher.exe...
start "" "DreamCatcher.exe"

