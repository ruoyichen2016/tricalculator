@echo off
setlocal
set DIR=%~dp0
set ROOT=%DIR%..
if "%PCGEN_WEB_BIN%"=="" (
  if exist "%DIR%pcgen-web.exe" (
    set BIN=%DIR%pcgen-web.exe
  ) else (
    set BIN=%ROOT%\target\release\pcgen-web.exe
  )
) else (
  set BIN=%PCGEN_WEB_BIN%
)
if "%PCGEN_DATA_ROOT%"=="" (
  if exist "%DIR%data\packs\rsrd35" (
    set DATA=%DIR%data\packs\rsrd35
  ) else if exist "%ROOT%\data\packs\rsrd35" (
    set DATA=%ROOT%\data\packs\rsrd35
  ) else (
    set DATA=%ROOT%\..\raw\pcgen-master
  )
) else (
  set DATA=%PCGEN_DATA_ROOT%
)
if "%PCGEN_CHARACTERS_ROOT%"=="" (
  set CHARACTERS=%ROOT%\characters
) else (
  set CHARACTERS=%PCGEN_CHARACTERS_ROOT%
)
if not exist "%BIN%" (
  echo pcgen-web.exe not found at %BIN%
  exit /b 1
)
if not exist "%DATA%" (
  echo data pack not found at %DATA%
  exit /b 1
)
echo data root:  %DATA%
echo characters: %CHARACTERS%
start "" "%BIN%" --data-root "%DATA%" --cohort-root "%ROOT%" --characters-root "%CHARACTERS%"
timeout /t 2 /nobreak >nul
start "" http://127.0.0.1:8787
