@echo off
REM Ensure pyo3-build.cfg exists for cargo builds.
REM This script is run during pixi activation.

set "SCRIPT_DIR=%~dp0"
set "REPO_ROOT=%SCRIPT_DIR%..\.."
set "CONFIG_FILE=%REPO_ROOT%\rerun_py\pyo3-build.cfg"

if not exist "%CONFIG_FILE%" (
    echo Generating %CONFIG_FILE%...
    call "%SCRIPT_DIR%uvpy.bat" "%REPO_ROOT%\scripts\generate_pyo3_config.py"
)
