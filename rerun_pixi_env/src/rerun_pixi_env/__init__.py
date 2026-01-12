"""Pixi environment setup utilities for Rerun."""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path


def get_pixi_project_root() -> Path:
    """Get the PIXI_PROJECT_ROOT, or fail if not set."""
    root = os.environ.get("PIXI_PROJECT_ROOT")
    if not root:
        print("ERROR: PIXI_PROJECT_ROOT not set", file=sys.stderr)
        sys.exit(1)
    return Path(root)


def get_scripts_dir() -> Path:
    """Get the scripts/pixi directory."""
    return get_pixi_project_root() / "scripts" / "pixi"


def _install_shim(shim_name: str, target_name: str) -> None:
    """
    Install a shim executable to scripts/pixi/.

    The shim is copied from the pip-installed entry point launcher.
    """
    scripts_dir = get_scripts_dir()
    scripts_dir.mkdir(parents=True, exist_ok=True)

    if sys.platform == "win32":
        target = scripts_dir / f"{target_name}.exe"
    else:
        target = scripts_dir / target_name

    if not target.exists():
        source = shutil.which(shim_name)
        if source:
            print(f"Installing {target_name} shim: {target}")
            shutil.copy(source, target)
            if sys.platform != "win32":
                target.chmod(0o755)
        else:
            print(f"ERROR: {shim_name} not found in PATH", file=sys.stderr)
            sys.exit(1)


def ensure_uv_shim() -> None:
    """
    Ensure the uv shim exists in scripts/pixi/.

    The shim wraps uv to unset CONDA_PREFIX, ensuring uv targets .venv
    instead of the pixi environment.
    """
    _install_shim("uv-shim", "uv")


def ensure_uvpy_shim() -> None:
    """
    Ensure the uvpy shim exists in scripts/pixi/.

    The shim runs "uv run python" to execute Python in the .venv environment.
    This is useful for subprocess calls that need to find the correct Python.
    """
    _install_shim("uvpy-shim", "uvpy")


def ensure_pyo3_build_cfg() -> None:
    """
    Ensure pyo3-build.cfg exists for cargo builds.

    Uses python from PATH to match what PYO3_PYTHON="python" resolves to.
    """
    from .pyo3_config import generate_config_file

    pixi_root = get_pixi_project_root()
    config_file = pixi_root / "rerun_py" / "pyo3-build.cfg"

    generate_config_file(config_file)
    print(f"Generated {config_file}")


def main() -> None:
    """Entry point for ensure-rerun-env command."""
    ensure_uv_shim()
    ensure_uvpy_shim()
    ensure_pyo3_build_cfg()


def uv_shim_main() -> None:
    """
    Entry point for uv-shim command.

    This is the actual uv wrapper that gets copied to scripts/pixi/uv.exe on Windows.
    It unsets CONDA_PREFIX and execs the real uv.
    """
    # Remove CONDA_PREFIX so uv targets .venv instead of pixi env
    os.environ.pop("CONDA_PREFIX", None)

    pixi_root = get_pixi_project_root()

    if sys.platform == "win32":
        real_uv = pixi_root / ".pixi" / "envs" / "default" / "Scripts" / "uv.exe"
    else:
        real_uv = pixi_root / ".pixi" / "envs" / "default" / "bin" / "uv"

    # Pass through all arguments
    args = [str(real_uv)] + sys.argv[1:]

    if sys.platform == "win32":
        # On Windows, no exec(), so subprocess and exit
        result = subprocess.run(args)
        sys.exit(result.returncode)
    else:
        # On Unix, exec replaces this process
        os.execv(str(real_uv), args)


def uvpy_shim_main() -> None:
    """
    Entry point for uvpy-shim command.

    This runs "uv run python" to execute Python in the .venv environment.
    It's a subprocess-friendly way to invoke the correct Python.
    """
    # Remove CONDA_PREFIX so uv targets .venv instead of pixi env
    os.environ.pop("CONDA_PREFIX", None)

    pixi_root = get_pixi_project_root()

    if sys.platform == "win32":
        real_uv = pixi_root / ".pixi" / "envs" / "default" / "Scripts" / "uv.exe"
    else:
        real_uv = pixi_root / ".pixi" / "envs" / "default" / "bin" / "uv"

    # Run "uv run python" with any additional arguments
    args = [str(real_uv), "run", "python"] + sys.argv[1:]

    if sys.platform == "win32":
        result = subprocess.run(args)
        sys.exit(result.returncode)
    else:
        os.execv(str(real_uv), args)


if __name__ == "__main__":
    main()
