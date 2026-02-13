"""
Generate pyo3-build.cfg for stable builds across different build environments.

This module queries the current Python interpreter and generates a config file
that pyo3-build-config can use, ensuring consistent builds whether invoked
via `maturin develop`, `uv sync`, or any other method.

Note: The `version` field is set to 3.10 (our abi3 minimum), not the actual
Python version, since we build with abi3-py310.
"""

from __future__ import annotations

import struct
import sys
import sysconfig
from pathlib import Path
from typing import Any


def get_python_config() -> dict[str, Any]:
    """Get Python configuration from the current interpreter."""
    config = sysconfig.get_config_vars()

    # Get implementation - pyo3 expects exact casing: "CPython" or "PyPy"
    impl_name = sys.implementation.name
    if impl_name == "cpython":
        implementation = "CPython"
    elif impl_name == "pypy":
        implementation = "PyPy"
    else:
        implementation = impl_name

    # For abi3 builds, version is the minimum supported version, not the actual Python version.
    # We use abi3-py310, so this should be 3.10 regardless of what Python is installed.
    version = "3.10"

    # Determine if shared library
    # Match pyo3's logic: shared if Windows, macOS framework, PyPy, or Py_ENABLE_SHARED
    is_windows = sys.platform == "win32"
    is_framework = bool(config.get("PYTHONFRAMEWORK"))
    is_pypy = impl_name == "pypy"
    py_enable_shared = bool(config.get("Py_ENABLE_SHARED", 0))
    shared = is_windows or is_framework or is_pypy or py_enable_shared

    # Get library name
    # For abi3 builds, use "python3" not version-specific name
    if sys.platform == "win32":
        lib_name = "python3"
    else:
        lib_name = config.get("LDLIBRARY", "").replace(".so", "").replace("lib", "", 1)
        if not lib_name:
            lib_name = f"python{sys.version_info.major}.{sys.version_info.minor}"

    # Get library directory - match pyo3-build-config's logic exactly:
    # On Windows: sys.base_prefix + "\\libs"
    # On Unix: sysconfig LIBDIR
    if sys.platform == "win32":
        lib_dir = str(Path(sys.base_prefix) / "libs")
    else:
        lib_dir = config.get("LIBDIR", "")
        if not lib_dir:
            lib_dir = str(Path(sys.base_prefix) / "lib")

    # Pointer width
    pointer_width = struct.calcsize("P") * 8

    # Build flags (empty for most builds)
    build_flags = ""

    # Python framework prefix (macOS only)
    python_framework_prefix = ""
    if sys.platform == "darwin":
        framework = config.get("PYTHONFRAMEWORK", "")
        if framework:
            python_framework_prefix = config.get("PYTHONFRAMEWORKPREFIX", "")

    return {
        "implementation": implementation,
        "version": version,
        "shared": shared,
        "lib_name": lib_name,
        "lib_dir": lib_dir,
        "pointer_width": pointer_width,
        "build_flags": build_flags,
        "python_framework_prefix": python_framework_prefix,
    }


def get_python_executable() -> Path:
    """Get the path to the Python executable (for the config file).

    This returns the current interpreter's executable, which should match
    what PYO3_PYTHON="python" resolves to when running under pixi.
    """
    return Path(sys.executable).resolve()


def generate_config_file(output_path: Path) -> bool:
    """Generate the pyo3-build.cfg file.

    Returns True if the file was written, False if it was already up to date.
    """
    config = get_python_config()
    python_path = get_python_executable()

    # abi3 is true since we target abi3-py310
    abi3 = "true"

    # Format booleans as lowercase
    shared = "true" if config["shared"] else "false"

    # Don't suppress link lines - setting this to true causes UnicodeDecodeError
    # on Windows when the linker emits binary error output.
    suppress_link_lines = "false"

    # Use str() to ensure proper path format for the platform
    lines = [
        f"implementation={config['implementation']}",
        f"version={config['version']}",
        f"shared={shared}",
        f"abi3={abi3}",
        f"lib_name={config['lib_name']}",
        f"lib_dir={config['lib_dir']}",
        f"executable={str(python_path)}",
        f"pointer_width={config['pointer_width']}",
        f"build_flags={config['build_flags']}",
        f"suppress_build_script_link_lines={suppress_link_lines}",
    ]

    # Only include python_framework_prefix if non-empty (macOS)
    if config["python_framework_prefix"]:
        lines.append(f"python_framework_prefix={config['python_framework_prefix']}")

    new_content = "\n".join(lines) + "\n"

    # Only write if contents changed to avoid triggering cargo rebuilds
    if output_path.exists():
        existing_content = output_path.read_text()
        if existing_content == new_content:
            return False

    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(new_content)
    return True
