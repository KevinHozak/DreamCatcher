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
echo [1/3] Checking local Ollama service...
curl -s http://127.0.0.1:11434/api/tags >nul 2>&1
if %errorlevel% neq 0 (
    echo     Starting Ollama in background...
    start /B "" "%LOCALAPPDATA%\Programs\Ollama\ollama.exe" serve
    timeout /t 2 /nobreak >nul
) else (
    echo     Ollama is running.
)

:: 2. Launch FastAPI Backend
echo [2/3] Launching FastAPI Backend on http://127.0.0.1:8080...
cd backend
start "DreamCatcher Backend" /min .venv\Scripts\uvicorn.exe main:app --host 127.0.0.1 --port 8080
cd ..

:: 3. Launch Vite Frontend
echo [3/3] Launching UI Cockpit on http://localhost:5173...
cd frontend
start "DreamCatcher Frontend" /min cmd /c "npm run dev"
cd ..

echo.
echo ========================================================
echo  Ready! Opening DreamCatcher in your browser...
echo ========================================================
timeout /t 3 /nobreak >nul
start http://localhost:5173
