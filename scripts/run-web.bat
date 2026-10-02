@echo off
setlocal EnableExtensions

set "DIR=%~dp0"
set "ROOT=%DIR%.."
set "BIN="
set "DATA="
set "CHARACTERS="

rem ---- locate pcgen-web.exe ----
if not "%PCGEN_WEB_BIN%"=="" (
    set "BIN=%PCGEN_WEB_BIN%"
) else (
    if exist "%DIR%pcgen-web.exe" (
        set "BIN=%DIR%pcgen-web.exe"
    ) else if exist "%ROOT%\target\release\pcgen-web.exe" (
        set "BIN=%ROOT%\target\release\pcgen-web.exe"
    )
)

if not defined BIN (
    echo [ERROR] pcgen-web.exe not found.
    echo.
    if exist "%DIR%pcgen-web" (
        echo The release package contains "%DIR%pcgen-web".
        echo This looks like a Linux build; it cannot run directly on Windows.
    )
    echo Please use a Windows release build, or build locally with:
    echo   cargo build --release -p pcgen-web
    echo.
    pause
    exit /b 1
)

if not exist "%BIN%" (
    echo [ERROR] pcgen-web binary not found: %BIN%
    echo.
    pause
    exit /b 1
)

rem ---- locate data pack ----
if not "%PCGEN_DATA_ROOT%"=="" (
    set "DATA=%PCGEN_DATA_ROOT%"
) else (
    if exist "%DIR%data\packs\rsrd35" (
        set "DATA=%DIR%data\packs\rsrd35"
    ) else if exist "%ROOT%\data\packs\rsrd35" (
        set "DATA=%ROOT%\data\packs\rsrd35"
    ) else (
        set "DATA=%ROOT%\..\raw\pcgen-master"
    )
)

if not exist "%DATA%" (
    echo [ERROR] data pack not found: %DATA%
    echo.
    echo Run scripts\package_datapack.py or set PCGEN_DATA_ROOT.
    echo.
    pause
    exit /b 1
)

rem ---- locate/create characters dir ----
if not "%PCGEN_CHARACTERS_ROOT%"=="" (
    set "CHARACTERS=%PCGEN_CHARACTERS_ROOT%"
) else (
    set "CHARACTERS=%ROOT%\characters"
)
if not exist "%CHARACTERS%" mkdir "%CHARACTERS%"

echo data root:  %DATA%
echo characters: %CHARACTERS%
echo starting:   %BIN%
echo.
echo Opening browser at http://127.0.0.1:8787 after 2 seconds...
start "" /b cmd /c "timeout /t 2 /nobreak >nul & start http://127.0.0.1:8787"

echo Starting PCGen-Rust Web server. Keep this window open; close it to stop.
"%BIN%" --data-root "%DATA%" --cohort-root "%ROOT%" --characters-root "%CHARACTERS%"

echo.
echo Server stopped.
pause
