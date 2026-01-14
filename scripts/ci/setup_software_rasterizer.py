"""
Sets up software rasterizers for CI.

Borrows heavily from wgpu's CI setup.
See https://github.com/gfx-rs/wgpu/blob/a8a91737b2d2f378976e292074c75817593a0224/.github/workflows/ci.yml#L10
In fact we're the exact same Mesa builds that wgpu produces,
see https://github.com/gfx-rs/ci-build

For macOS, we use SwiftShader instead of lavapipe. SwiftShader is Google's software Vulkan implementation
that provides better compatibility with macOS.
Using a software rasterizer avoids GPU-related flakiness on CI runners which we hit quite often when
running many tests in parallel - we got spurious timeouts and failure to find graphics devices,
the cause of these issues is unknown.
(Since SwiftShader is Apache 2.0 licensed, we can host the binaries ourselves, which speeds up the whole process.)
"""

from __future__ import annotations

import json
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

# SwiftShader version for macOS software rasterization
# This corresponds to the Chrome version from which the binaries were extracted
SWIFTSHADER_VERSION = "144.0.7559.60"

# GCloud bucket for SwiftShader binaries
SWIFTSHADER_GCLOUD_BUCKET = "rerun-test-assets"
SWIFTSHADER_GCLOUD_PATH = f"swiftshader/{SWIFTSHADER_VERSION}"

CARGO_TARGET_DIR = Path(os.environ.get("CARGO_TARGET_DIR", "target"))


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
    copytree("mesa", CARGO_TARGET_DIR / "debug")
    copytree("mesa", CARGO_TARGET_DIR / "debug" / "deps")

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
    # See: https://github.com/KhronosGroup/Vulkan-Loader/blob/vulkan-sdk-1.4.304/docs/LoaderInterfaceArchitecture.md#elevated-privilege-caveats
    # Therefore, we have to set one of the registry keys that is checked to find the driver.
    # See: https://vulkan.lunarg.com/doc/view/1.4.304.1/windows/LoaderDriverInterface.html#driver-discovery-on-windows

    # Write registry keys to configure Vulkan drivers
    import winreg

    key_path = "SOFTWARE\\Khronos\\Vulkan\\Drivers"

    # mypy apparently doesn't have stubs for wingreg
    key = winreg.CreateKeyEx(winreg.HKEY_LOCAL_MACHINE, key_path)  # type: ignore[attr-defined]
    winreg.SetValueEx(key, icd_json_path_str, 0, winreg.REG_DWORD, 0)  # type: ignore[attr-defined]
    winreg.CloseKey(key)  # type: ignore[attr-defined]

    return env_vars


def extract_swiftshader_from_chrome() -> tuple[Path, str] | None:
    """
    Extract SwiftShader binaries from a local (!) Chrome installation.

    This is primarily used to create the initial upload to GCloud storage.
    Chrome bundles SwiftShader (Apache 2.0 license) as its software Vulkan implementation.

    Returns:
        Tuple of (Path to libvk_swiftshader.dylib, Chrome version string), or None if Chrome is not found.
    """
    chrome_paths = [
        "/Applications/Google Chrome.app",
        "/Applications/Chromium.app",
    ]

    for chrome_path in chrome_paths:
        chrome_base = Path(chrome_path)
        if not chrome_base.exists():
            continue

        # Find the latest version directory
        framework_path = chrome_base / "Contents/Frameworks"
        if "Chrome" in chrome_path:
            framework_path = framework_path / "Google Chrome Framework.framework/Versions"
        else:
            framework_path = framework_path / "Chromium Framework.framework/Versions"

        if not framework_path.exists():
            continue

        # Find the latest version (skip symlinks like "Current")
        versions = [d for d in framework_path.iterdir() if d.is_dir() and not d.is_symlink()]
        if not versions:
            continue

        latest_version = max(versions, key=lambda p: p.name)
        swiftshader_lib = latest_version / "Libraries/libvk_swiftshader.dylib"

        if swiftshader_lib.exists():
            version_str = latest_version.name
            print(f"Found SwiftShader in {chrome_path}, version {version_str}")
            print(f"Library path: {swiftshader_lib}")
            return swiftshader_lib, version_str

    return None


def upload_swiftshader_to_gcloud(lib_path: Path, version: str) -> None:
    """
    Upload SwiftShader binary to GCloud storage.

    This is a helper function for maintainers to upload new SwiftShader versions.
    Run this script with --upload-swiftshader to extract from Chrome and upload.

    SwiftShader is Apache 2.0 licensed, so we can redistribute the binaries.
    See: https://github.com/google/swiftshader/blob/master/LICENSE.txt

    Args:
        lib_path: Path to the libvk_swiftshader.dylib file to upload
        version: Chrome version string (e.g. "143.0.7499.193")
    """
    try:
        from google.cloud import storage
    except ImportError:
        print("ERROR: google-cloud-storage is not installed.")
        print("Install it with: pip install google-cloud-storage")
        sys.exit(1)

    blob_path = f"swiftshader/{version}/libvk_swiftshader.dylib"
    gcloud_url = f"gs://{SWIFTSHADER_GCLOUD_BUCKET}/{blob_path}"

    print(f"\nUploading {lib_path} to {gcloud_url}")
    try:
        client = storage.Client("rerun-open")
        bucket = client.bucket(SWIFTSHADER_GCLOUD_BUCKET)
        blob = bucket.blob(blob_path)

        print(f"Uploading to bucket '{SWIFTSHADER_GCLOUD_BUCKET}' at path '{blob_path}'...")
        blob.upload_from_filename(str(lib_path))

        print(f"✓ Successfully uploaded to {gcloud_url}")

        if version != SWIFTSHADER_VERSION:
            print(f"\nNext step: Update SWIFTSHADER_VERSION = '{version}' in this script to use the new version in CI.")
    except Exception as e:
        print(f"✗ Upload failed: {e}")
        print("Make sure you're authenticated with: gcloud auth application-default login")
        sys.exit(1)


def setup_swiftshader_for_macos() -> dict[str, str]:
    """
    Sets up SwiftShader software rasterizer for macOS.

    SwiftShader is Google's software Vulkan implementation (Apache 2.0 licensed).
    We use it for CI testing to avoid GPU-related flakiness on macOS runners.

    This function:
    1. Downloads libvk_swiftshader.dylib from GCloud storage
    2. Creates a Vulkan ICD (Installable Client Driver) JSON file pointing to the library
    3. Sets VK_DRIVER_FILES environment variable to use SwiftShader

    Note: The Vulkan SDK must be installed separately (for the loader and vulkaninfo).

    Returns:
        Dictionary of environment variables to set.
    """
    print("Setting up SwiftShader for macOS...")

    try:
        from google.cloud import storage
    except ImportError:
        print("ERROR: google-cloud-storage is not installed.")
        print("Install it with: pip install google-cloud-storage")
        sys.exit(1)

    # Create directory for SwiftShader
    swiftshader_dir = Path.home() / "swiftshader"
    swiftshader_dir.mkdir(exist_ok=True)

    swiftshader_lib = swiftshader_dir / "libvk_swiftshader.dylib"

    # Download from GCloud
    blob_path = f"{SWIFTSHADER_GCLOUD_PATH}/libvk_swiftshader.dylib"
    gcloud_url = f"gs://{SWIFTSHADER_GCLOUD_BUCKET}/{blob_path}"
    print(f"Downloading SwiftShader from {gcloud_url}...")

    try:
        client = storage.Client("rerun-open")
        bucket = client.bucket(SWIFTSHADER_GCLOUD_BUCKET)
        blob = bucket.blob(blob_path)

        if not blob.exists():
            print(f"✗ SwiftShader binary not found at {gcloud_url}")
            print("If you're a maintainer, upload it with:")
            print(f"  python {__file__} --upload-swiftshader")
            sys.exit(1)

        blob.download_to_filename(str(swiftshader_lib))
        print(f"✓ Downloaded SwiftShader to {swiftshader_lib}")

    except Exception as e:
        print(f"✗ GCloud download failed: {e}")
        sys.exit(1)

    # Create ICD JSON file
    # The ICD file tells the Vulkan loader where to find the driver implementation.
    # See: https://vulkan.lunarg.com/doc/view/latest/mac/loader_and_layer_interface.html
    icd_json = {
        "file_format_version": "1.0.0",
        "ICD": {"library_path": str(swiftshader_lib.resolve()), "api_version": "1.3.0"},
    }

    icd_json_path = swiftshader_dir / "vk_swiftshader_icd.json"

    with open(icd_json_path, "w", encoding="utf-8") as f:
        json.dump(icd_json, f, indent=2)

    print(f"✓ Created ICD file at {icd_json_path}")

    # Set environment variables
    env_vars = {
        "VK_DRIVER_FILES": str(icd_json_path.resolve()),
    }
    set_environment_variables(env_vars)

    return env_vars


def vulkan_info(extra_env_vars: dict[str, str]) -> None:
    """Run vulkaninfo to verify the Vulkan setup."""
    vulkan_sdk_path = os.environ["VULKAN_SDK"]

    env = os.environ.copy()
    env["VK_LOADER_DEBUG"] = "all"  # Enable verbose logging of vulkan loader for debugging.
    for key, value in extra_env_vars.items():
        env[key] = value

    if os.name == "nt":
        vulkaninfo_path = f"{vulkan_sdk_path}/bin/vulkaninfoSDK.exe"
    elif sys.platform == "darwin":
        vulkaninfo_path = f"{vulkan_sdk_path}/macOS/bin/vulkaninfo"
    else:
        vulkaninfo_path = f"{vulkan_sdk_path}/bin/vulkaninfo"

    if not Path(vulkaninfo_path).exists():
        print(f"ERROR: vulkaninfo not found at {vulkaninfo_path}")
        print("The Vulkan SDK should be installed with vulkaninfo utility.")
        sys.exit(1)

    print(f"\nRunning vulkaninfo to verify setup (from {vulkaninfo_path})...\n")
    print(run([vulkaninfo_path, "--summary"], env=env).stdout)


def check_for_vulkan_sdk() -> None:
    vulkan_sdk_path = os.environ.get("VULKAN_SDK")
    if vulkan_sdk_path is None:
        print(
            "ERROR: VULKAN_SDK is not set. The sdk needs to be installed prior including runtime & vulkaninfo utility.",
        )
        sys.exit(1)


def main() -> None:
    # Handle --upload-swiftshader flag for maintainers
    if len(sys.argv) > 1 and sys.argv[1] == "--upload-swiftshader":
        if sys.platform != "darwin":
            print("ERROR: --upload-swiftshader is only supported on macOS")
            sys.exit(1)

        print("Extracting SwiftShader from local Chrome and uploading to GCloud...")
        result = extract_swiftshader_from_chrome()
        if result is None:
            print("ERROR: Could not find SwiftShader in Chrome installation")
            sys.exit(1)

        lib_path, version = result
        upload_swiftshader_to_gcloud(lib_path, version)
        return

    # We only use Vulkan software rasterizers right now.
    check_for_vulkan_sdk()

    # Normal setup
    if os.name == "nt" and platform.machine() == "AMD64":
        # Note that we could also use WARP, the DX12 software rasterizer.
        # (wgpu tests with both llvmpip and WARP)
        # But practically speaking we prefer Vulkan anyways on Windows today and as such this is
        # both less variation and closer to what Rerun uses when running on a "real" machine.
        env_vars = setup_lavapipe_for_windows()
        vulkan_info(env_vars)
    elif os.name == "posix" and sys.platform != "darwin" and platform.machine() == "x86_64":
        env_vars = setup_lavapipe_for_linux()
        vulkan_info(env_vars)
    elif os.name == "posix" and sys.platform == "darwin":
        env_vars = setup_swiftshader_for_macos()
        vulkan_info(env_vars)
    else:
        raise ValueError(f"Unsupported OS / architecture: {os.name} / {platform.machine()}")


if __name__ == "__main__":
    main()
