@echo off
title VIctory Launcher
cd /d "%~dp0"

:: Pass the game executable as an argument, drag-and-drop it onto this script, or enter path below:
set "GAME_EXE=%~1"
if "%GAME_EXE%"=="" (
    set /p GAME_EXE="Enter full path to game_scarlett_master.exe: "
)

:: Strip surrounding quotes if present
set "GAME_EXE=%GAME_EXE:"=%"

if not exist "%GAME_EXE%" (
    echo.
    echo Error: Game executable not found at: "%GAME_EXE%"
    echo Please make sure the path is correct.
    echo.
    pause
    exit /b 1
)

set VICTORY_LOG=info
set VICTORY_AMBIENT=1.2
set VICTORY_TEXTURE_DROP_MIPS=1
set VICTORY_CARD_WINDOWS_MB=1024
set VICTORY_LOOKUP_CACHE=1
set VICTORY_COPY_BACK_ONLY_READBACK=0
set VICTORY_INDIRECT_GATHER_ELIDE=0
set VICTORY_RELAY=2
set VICTORY_LATE_WAIT=3
set VICTORY_WIDE_BACK=1
set VICTORY_INDIRECT_WINDOW=1
set VICTORY_INDIRECT_GATHER=1
set VICTORY_INDIRECT_CARD_BUDGET=1
set VICTORY_NO_WATCHDOG=1
set VICTORY_DRIVER_WATCH=0
set VICTORY_SHADER_LOOP_LIMIT=1024

echo Starting VIctory for "%GAME_EXE%"...
.\target\diag\victory.exe run "%GAME_EXE%" --execute
pause
