"""
Sets up software rasterizers for CI.

Borrows heavily from wgpu's CI setup.
See https://github.com/gfx-rs/wgpu/blob/a8a91737b2d2f378976e292074c75817593a0224/.github/workflows/ci.yml#L10
In fact we're the exact same Mesa builds that wgpu produces,
see https://github.com/gfx-rs/ci-build
"""

from __future__ import annotations

import os
import platform
import subprocess
import sys
from distutils.dir_util import copy_tree
from pathlib import Path

# Sourced from https://archive.mesa3d.org/. Bumping this requires
# updating the mesa build in https://github.com/gfx-rs/ci-build and creating a new release.
MESA_VERSION = "24.2.3"

# Corresponds to https://github.com/gfx-rs/ci-build/releases
CI_BINARY_BUILD = "build19"

TARGET_DIR = Path("target/debug")


def run(
    args: list[str], *, env: dict[str, str] | None = None, timeout: int | None = None, cwd: str | None = None
) -> None:
    print(f"> {subprocess.list2cmdline(args)}")
    result = subprocess.run(args, env=env, cwd=cwd, timeout=timeout, check=False, capture_output=True, text=True)
    assert (
        result.returncode == 0
    ), f"{subprocess.list2cmdline(args)} failed with exit-code {result.returncode}. Output:\n{result.stdout}\n{result.stderr}"


def set_environment_variables(variables: dict[str, str]) -> None:
    """
    Sets environment variables in the GITHUB_ENV file.

    If `GITHUB_ENV` is not set (i.e. when running locally), prints the variables to stdout.
    """

    github_env = os.environ.get("GITHUB_ENV")
    if github_env is None:
        print(f"GITHUB_ENV is not set. The following environment variables need to be set:\n{variables}")
    else:
        print(f"Setting environment variables in {github_env}:\n{variables}")
        with open(github_env, "a", encoding="utf-8") as f:
            for key, value in variables.items():
                f.write(f"{key}={value}\n")


def setup_lavapipe_for_linux() -> None:
    """Sets up lavapipe mesa driver for Linux (x64)."""
    # Download mesa
    run([
        "curl",
        "-L",
        "--retry",
        "5",
        f"https://github.com/gfx-rs/ci-build/releases/download/{CI_BINARY_BUILD}/mesa-{MESA_VERSION}-linux-x86_64.tar.xz",
        "-o",
        "mesa.tar.xz",
    ])

    # Create mesa directory and extract
    os.makedirs("mesa", exist_ok=True)
    run(["tar", "xpf", "mesa.tar.xz", "-C", "mesa"])

    # The ICD provided by the mesa build is hardcoded to the build environment.
    # We write out our own ICD file to point to the mesa vulkan
    icd_json = f"""{{
    "ICD": {{
        "api_version": "1.1.255",
        "library_path": "{os.getcwd()}/mesa/lib/x86_64-linux-gnu/libvulkan_lvp.so"
    }},
    "file_format_version": "1.0.0"
}}"""
    with open("icd.json", "w", encoding="utf-8") as f:
        f.write(icd_json)

    # Update environment variables
    set_environment_variables({
        "VK_DRIVER_FILES": f"{os.getcwd()}/icd.json",
        "LD_LIBRARY_PATH": f"{os.getcwd()}/mesa/lib/x86_64-linux-gnu/:{os.environ.get('LD_LIBRARY_PATH', '')}"
    })


def setup_lavapipe_for_windows() -> None:
    """Sets up lavapipe mesa driver for Windows (x64)."""

    # Download mesa
    run([
        "curl.exe",
        "-L",
        "--retry",
        "5",
        f"https://github.com/pal1000/mesa-dist-win/releases/download/{MESA_VERSION}/mesa3d-{MESA_VERSION}-release-msvc.7z",
        "-o",
        "mesa.7z",
    ])

    # Extract needed files
    run([
        "7z.exe",
        "e",
        "mesa.7z",
        "-aoa",
        "-omesa",
        "x64/vulkan_lvp.dll",
        "x64/lvp_icd.x86_64.json",
    ])

    # Copy files to target directory.
    copy_tree("mesa", TARGET_DIR)
    copy_tree("mesa", TARGET_DIR / "deps")

    # Set environment variables, make sure to use windows path style.
    mesa_json_path = (
        Path(os.path.join(os.getcwd(), "mesa", "lvp_icd.x86_64.json")).resolve().as_posix().replace("/", "\\")
    )
    set_environment_variables({"VK_DRIVER_FILES": mesa_json_path})


def main() -> None:
    if os.name == "nt" and platform.machine() == "AMD64":
        # Note that we could also use WARP, the DX12 software rasterizer.
        # (wgpu tests with both llvmpip and WARP)
        # But practically speaking we prefer Vulkan anyways on Windows today and as such this is
        # both less variation and closer to what Rerun uses when running on a "real" machine.
        setup_lavapipe_for_windows()
    elif os.name == "posix" and sys.platform != "darwin" and platform.machine() == "x86_64":
        setup_lavapipe_for_linux()
    elif os.name == "posix" and sys.platform == "darwin":
        pass  # We don't have a software rasterizer for macOS.
    else:
        raise ValueError(f"Unsupported OS / architecture: {os.name} / {platform.machine()}")


if __name__ == "__main__":
    main()
