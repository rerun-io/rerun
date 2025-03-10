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
import shutil
import subprocess
import sys
from pathlib import Path
from shutil import copytree

# Sourced from https://archive.mesa3d.org/. Bumping this requires
# updating the mesa build in https://github.com/gfx-rs/ci-build and creating a new release.
MESA_VERSION = "24.2.3"

# Corresponds to https://github.com/gfx-rs/ci-build/releases
CI_BINARY_BUILD = "build19"

TARGET_DIR = Path("target/debug")


def run(
    args: list[str],
    *,
    env: dict[str, str] | None = None,
    timeout: int | None = None,
    cwd: str | None = None,
) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(args, env=env, cwd=cwd, timeout=timeout, check=False, capture_output=True, text=True)
    assert result.returncode == 0, (
        f"{subprocess.list2cmdline(args)} failed with exit-code {result.returncode}. Output:\n{result.stdout}\n{result.stderr}"
    )
    return result


def set_environment_variables(variables: dict[str, str]) -> None:
    """
    Sets environment variables in the GITHUB_ENV file.

    If `GITHUB_ENV` is not set (i.e. when running locally), prints the variables to stdout.
    """
    for key, value in variables.items():
        os.environ[key] = value

    # Set in GITHUB_ENV file.
    github_env = os.environ.get("GITHUB_ENV")
    if github_env is None:
        print(f"GITHUB_ENV is not set. The following environment variables need to be set:\n{variables}")
    else:
        print(f"Setting environment variables in {github_env}:\n{variables}")
        # Write to GITHUB_ENV file.
        with open(github_env, "a", encoding="utf-8") as f:
            for key, value in variables.items():
                f.write(f"{key}={value}\n")


def setup_lavapipe_for_linux() -> dict[str, str]:
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
    icd_json_path = Path("icd.json")
    with open(icd_json_path, "w", encoding="utf-8") as f:
        f.write(icd_json)

    # Update environment variables
    env_vars = {
        "VK_DRIVER_FILES": f"{os.getcwd()}/{icd_json_path}",
        "LD_LIBRARY_PATH": f"{os.getcwd()}/mesa/lib/x86_64-linux-gnu/:{os.environ.get('LD_LIBRARY_PATH', '')}",
    }
    set_environment_variables(env_vars)

    # On CI we run with elevated privileges, therefore VK_DRIVER_FILES is ignored.
    # See: https://github.com/KhronosGroup/Vulkan-Loader/blob/sdk-1.3.261/docs/LoaderInterfaceArchitecture.md#elevated-privilege-caveats
    # (curiously, when installing the Vulkan SDK via apt, it seems to work fine).
    # Therefore, we copy the icd file into one of the standard search paths.
    target_path = Path("~/.config/vulkan/icd.d").expanduser()
    print(f"Copying icd file to {target_path}")
    target_path.mkdir(parents=True, exist_ok=True)
    shutil.copy(icd_json_path, target_path)

    return env_vars


def setup_lavapipe_for_windows() -> dict[str, str]:
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
    copytree("mesa", TARGET_DIR)
    copytree("mesa", TARGET_DIR / "deps")

    # Print icd file that should be used.
    icd_json_path = Path(os.path.join(os.getcwd(), "mesa", "lvp_icd.x86_64.json")).resolve()
    print(f"Using ICD file at '{icd_json_path}':")
    with open(icd_json_path, encoding="utf-8") as f:
        print(f.read())
    icd_json_path_str = icd_json_path.as_posix().replace("/", "\\")

    # Set environment variables, make sure to use windows path style.
    vulkan_runtime_path = f"{os.environ['VULKAN_SDK']}/runtime/x64"
    env_vars = {
        "VK_DRIVER_FILES": icd_json_path_str,
        # Vulkan runtime install should do this, but the CI action we're using right now for instance doesn't,
        # causing `vulkaninfo` to fail since it can't find the vulkan loader.
        "PATH": f"{os.environ.get('PATH', '')};{vulkan_runtime_path}",
    }
    set_environment_variables(env_vars)

    # For debugging: List files in Vulkan runtime path.
    if False:
        print(f"\nListing files in Vulkan runtime path '{vulkan_runtime_path}':")
        try:
            files = os.listdir(vulkan_runtime_path)
            for file in files:
                print(f"  {file}")
        except Exception as e:
            print(f"Error listing Vulkan runtime directory: {e}")

    # On CI we run with elevated privileges, therefore VK_DRIVER_FILES is ignored.
    # See: https://github.com/KhronosGroup/Vulkan-Loader/blob/sdk-1.3.261/docs/LoaderInterfaceArchitecture.md#elevated-privilege-caveats
    # Therefore, we have to set one of the registry keys that is checked to find the driver.
    # See: https://vulkan.lunarg.com/doc/view/1.3.243.0/windows/LoaderDriverInterface.html#user-content-driver-discovery-on-windows

    # Write registry keys to configure Vulkan drivers
    import winreg

    key_path = "SOFTWARE\\Khronos\\Vulkan\\Drivers"

    # mypy apparently doesn't have stubs for wingreg
    key = winreg.CreateKeyEx(winreg.HKEY_LOCAL_MACHINE, key_path)  # type: ignore[attr-defined]
    winreg.SetValueEx(key, icd_json_path_str, 0, winreg.REG_DWORD, 0)  # type: ignore[attr-defined]
    winreg.CloseKey(key)  # type: ignore[attr-defined]

    return env_vars


def vulkan_info(extra_env_vars: dict[str, str]) -> None:
    vulkan_sdk_path = os.environ["VULKAN_SDK"]
    env = os.environ.copy()
    env["VK_LOADER_DEBUG"] = "all"  # Enable verbose logging of vulkan loader for debugging.
    for key, value in extra_env_vars.items():
        env[key] = value

    if os.name == "nt":
        vulkaninfo_path = f"{vulkan_sdk_path}/bin/vulkaninfoSDK.exe"
    else:
        vulkaninfo_path = f"{vulkan_sdk_path}/bin/vulkaninfo"
    print(run([vulkaninfo_path], env=env).stdout)


def check_for_vulkan_sdk() -> None:
    vulkan_sdk_path = os.environ.get("VULKAN_SDK")
    if vulkan_sdk_path is None:
        print(
            "ERROR: VULKAN_SDK is not set. The sdk needs to be installed prior including runtime & vulkaninfo utility.",
        )
        sys.exit(1)


def main() -> None:
    if os.name == "nt" and platform.machine() == "AMD64":
        # Note that we could also use WARP, the DX12 software rasterizer.
        # (wgpu tests with both llvmpip and WARP)
        # But practically speaking we prefer Vulkan anyways on Windows today and as such this is
        # both less variation and closer to what Rerun uses when running on a "real" machine.
        check_for_vulkan_sdk()
        env_vars = setup_lavapipe_for_windows()
        vulkan_info(env_vars)
    elif os.name == "posix" and sys.platform != "darwin" and platform.machine() == "x86_64":
        check_for_vulkan_sdk()
        env_vars = setup_lavapipe_for_linux()
        vulkan_info(env_vars)
    elif os.name == "posix" and sys.platform == "darwin":
        print("Skipping software rasterizer setup for macOS - we have to rely on a real GPU here.")
    else:
        raise ValueError(f"Unsupported OS / architecture: {os.name} / {platform.machine()}")


if __name__ == "__main__":
    main()
