#!/usr/bin/env python3

"""
Install a wheel from a folder in the specified pixi environment.

Example:
-------
```
python3 scripts/ci/pixi_install_wheel.py --feature python-pypi --dir wheel --package rerun-sdk
```

"""

from __future__ import annotations

import argparse
import os
import platform
import subprocess
import sys
from pathlib import Path


def run_pixi_install(feature: str, dir: str, pkg: str) -> None:
    # Find our current platform: linux, macosx, or win
    plat = platform.system()
    if plat == "Linux":
        plat = "manylinux"
    elif plat == "Darwin":
        plat = "macosx"
    elif plat == "Windows":
        plat = "win"

    arch = os.uname().machine

    # Find the wheels
    wheels = [whl.name for whl in Path(dir).glob("*.whl")]

    # Filter the wheels based on package
    wheels = [whl for whl in wheels if whl.startswith(pkg.replace("-", "_"))]

    # Filter the wheels based on platform
    wheels = [whl for whl in wheels if plat in whl]

    # Filter the wheels based on architecture
    wheels = [whl for whl in wheels if arch in whl]

    if len(wheels) == 0:
        print(f"No wheels found for package {pkg} on platform {plat} and architecture {arch}")
        sys.exit(1)

    if len(wheels) > 1:
        print(f"Multiple wheels found for package {pkg} on platform {plat} and architecture {arch}: {wheels}")
        sys.exit(1)

    wheel = Path(dir) / wheels[0]

    # Install the wheel
    cmd = ["pixi", "add", "--feature", feature, "--pypi", f"{pkg} @ {wheel}"]
    print(f"Running: {' '.join(cmd)}")
    returncode = subprocess.Popen(cmd).wait()
    assert returncode == 0, f"process exited with error code {returncode}"

    print(f"Wheel installed: {wheel.name}")


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Find and install a wheel from a folder in the specified pixi environment"
    )
    parser.add_argument("--feature", required=True, help="The pixi feature to update")
    parser.add_argument("--dir", required=True, help="Directory to search")
    parser.add_argument("--package", required=True, help="The package to install")
    args = parser.parse_args()

    run_pixi_install(args.feature, args.dir, args.package)


if __name__ == "__main__":
    main()
