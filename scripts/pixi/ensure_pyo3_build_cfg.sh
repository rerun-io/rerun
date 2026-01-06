#!/usr/bin/env bash
# Ensure pyo3-build.cfg exists for cargo builds.
# This script is run during pixi activation.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
CONFIG_FILE="${REPO_ROOT}/rerun_py/pyo3-build.cfg"

if [ ! -f "${CONFIG_FILE}" ]; then
    echo "Generating ${CONFIG_FILE}..."
    # Use uvpy to run with the .venv Python
    "${SCRIPT_DIR}/uvpy" "${REPO_ROOT}/scripts/generate_pyo3_config.py"
fi
