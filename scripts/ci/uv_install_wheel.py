#!/usr/bin/env python3

"""
Install a wheel from a folder in the specified pixi environment.

Example:
-------
```
python3 scripts/ci/uv_install_wheel.py --dir wheel --package rerun-sdk
```

"""

from __future__ import annotations

import argparse
import os
import platform
import subprocess
import sys
from pathlib import Path
from urllib.parse import urljoin
from urllib.request import pathname2url


def path_to_file_url(path: Path) -> str:
    return urljoin("file:", pathname2url(str(path.absolute())))


def run_uv_install(dir: str, pkg: str, platform_independent: bool = False) -> None:
    # Find our current platform: linux, macosx, or win
    plat = platform.system()
    if plat == "Linux":
        plat = "manylinux"
    elif plat == "Darwin":
        plat = "macosx"
    elif plat == "Windows":
        plat = "win"

    if hasattr(os, "uname"):
        arch = os.uname().machine
    else:
        arch = platform.machine().lower()

    print(f"Platform: {plat}, Architecture: {arch}")

    # Find the wheels
    wheels = [whl.name for whl in Path(dir).glob("*.whl")]
    print(f"Found {len(wheels)} wheels: {wheels}")

    # Filter the wheels based on package
    wheels = [whl for whl in wheels if whl.startswith(pkg.replace("-", "_"))]

    # Filter the wheels based on platform
    if not platform_independent:
        wheels = [whl for whl in wheels if plat in whl]

    # Filter the wheels based on architecture
    if not platform_independent:
        wheels = [whl for whl in wheels if arch in whl]

    if len(wheels) == 0:
        if platform_independent:
            print(f"No wheels found for package {pkg} (the package was expected to be platform independent)")
        else:
            print(f"No wheels found for package {pkg} on platform {plat} and architecture {arch}")
        sys.exit(1)

    if len(wheels) > 1:
        if platform_independent:
            print(
                f"Multiple wheels found for package {pkg} (the package was expected to be platform independent): {wheels}",
            )
        else:
            print(f"Multiple wheels found for package {pkg} on platform {plat} and architecture {arch}: {wheels}")
        sys.exit(1)

    wheel = Path(dir) / wheels[0]

    # Bootstrap the uv environment if it's missing
    cmd = ["pixi", "run", "uv", "sync", "--inexact"]
    print(f"Running: {' '.join(cmd)}")
    returncode = subprocess.Popen(cmd).wait()
    assert returncode == 0, f"process exited with error code {returncode}"

    # Install the wheel
    cmd = ["uv", "pip", "install", f"{pkg}[tests] @ {path_to_file_url(wheel)}"]
    print(f"Running: {' '.join(cmd)}")
    returncode = subprocess.Popen(cmd).wait()
    assert returncode == 0, f"process exited with error code {returncode}"

    print(f"Wheel installed: {wheel.name}")


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Find and install a wheel from a folder in the specified pixi environment",
    )
    parser.add_argument("--dir", required=True, help="Directory to search")
    parser.add_argument("--package", required=True, help="The package to install")
    parser.add_argument(
        "--platform-independent",
        action="store_true",
        help="Specify if the wheel is platform independent and there should be no filtering for architecture & platform",
    )
    args = parser.parse_args()

    run_uv_install(args.dir, args.package, args.platform_independent)


if __name__ == "__main__":
    main()
