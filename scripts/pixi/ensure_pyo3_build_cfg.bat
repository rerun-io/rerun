:<<'BATCH_SCRIPT'
@echo off
REM Polyglot script: works in cmd.exe and bash/Git Bash
REM Ensures pyo3-build.cfg exists for cargo builds.

set "SCRIPT_DIR=%~dp0"
set "REPO_ROOT=%SCRIPT_DIR%..\..\"
set "CONFIG_FILE=%REPO_ROOT%rerun_py\pyo3-build.cfg"

if not exist "%CONFIG_FILE%" (
    echo Generating %CONFIG_FILE% ...
    REM Use python from PATH to match what PYO3_PYTHON="python" resolves to.
    python "%REPO_ROOT%scripts\generate_pyo3_config.py"
)
goto :eof
BATCH_SCRIPT

# Bash section - runs when executed by bash/Git Bash
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
CONFIG_FILE="${REPO_ROOT}/rerun_py/pyo3-build.cfg"

if [ ! -f "${CONFIG_FILE}" ]; then
    echo "Generating ${CONFIG_FILE} ..."
    # Use python from PATH to match what PYO3_PYTHON="python" resolves to.
    python "${REPO_ROOT}/scripts/generate_pyo3_config.py"
fi
