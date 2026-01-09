:<<'BATCH_SCRIPT'
@echo off
REM Polyglot script: works in cmd.exe and bash/Git Bash
REM Pixi activation script for Windows.
REM Runs ensure-rerun-env to set up the environment.

REM ensure-rerun-env may not exist yet on first activation (before package install).
REM In that case, silently skip - it will run on next activation after install.
where ensure-rerun-env >nul 2>nul
if %errorlevel%==0 (
    ensure-rerun-env
)
goto :eof
BATCH_SCRIPT

# Bash section - runs when executed by bash/Git Bash on Windows
if command -v ensure-rerun-env &> /dev/null; then
    ensure-rerun-env
fi
