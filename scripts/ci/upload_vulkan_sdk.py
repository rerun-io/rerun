"""
Upload Vulkan SDK artifacts to GCloud storage for CI.

This script downloads Vulkan SDK installers from LunarG and uploads them to
the rerun-test-assets GCloud bucket so they can be used by the
rerun-io/install-vulkan-sdk-action in CI.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

# Version to upload
VULKAN_VERSION = "1.3.290.0"

# GCloud bucket for Vulkan SDK binaries
GCLOUD_BUCKET = "rerun-test-assets"


def run(args: list[str]) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(args, check=False, capture_output=True, text=True)
    assert result.returncode == 0, (
        f"{subprocess.list2cmdline(args)} failed with exit-code {result.returncode}. Output:\n{result.stdout}\n{result.stderr}"
    )
    return result


def download_and_upload_vulkan_sdk(version: str, platform: str) -> None:
    """
    Download Vulkan SDK from LunarG and upload to GCloud.

    Args:
        version: Vulkan SDK version (e.g. "1.3.290.0")
        platform: Platform name ("mac", "linux", "windows")
    """
    try:
        from google.cloud import storage
    except ImportError:
        print("ERROR: google-cloud-storage is not installed.")
        print("Install it with: pip install google-cloud-storage")
        sys.exit(1)

    # Determine file extension and download URL based on platform and version
    if platform == "mac":
        # For versions up to 1.3.290.0 use .dmg, after that use .zip
        # Since this is 1.3.290.0, we use .dmg
        extension = "dmg"
        filename = f"vulkansdk-macos-{version}.{extension}"
        lunarg_url = f"https://sdk.lunarg.com/sdk/download/{version}/mac/{filename}"
    elif platform == "linux":
        extension = "tar.xz"
        filename = f"vulkansdk-linux-x86_64-{version}.{extension}"
        lunarg_url = f"https://sdk.lunarg.com/sdk/download/{version}/linux/{filename}"
    elif platform == "windows":
        extension = "exe"
        filename = f"vulkansdk-windows-X64-{version}.{extension}"
        lunarg_url = f"https://sdk.lunarg.com/sdk/download/{version}/windows/{filename}"
    else:
        print(f"ERROR: Unsupported platform: {platform}")
        sys.exit(1)

    local_file = Path(filename)

    print(f"\nDownloading {filename} from LunarG…")
    print(f"   URL: {lunarg_url}")

    # Download from LunarG
    try:
        run(["curl", "-L", "--retry", "5", lunarg_url, "-o", str(local_file)])
        print(f"OK: Downloaded to {local_file}")
    except AssertionError as e:
        print(f"ERROR: Download failed: {e}")
        print("\nPossible reasons:")
        print(f"  1. The version {version} doesn't exist on LunarG's servers")
        print("  2. The filename format has changed")
        print("  3. Network connectivity issues")
        sys.exit(1)

    # Verify file exists and has reasonable size
    if not local_file.exists():
        print(f"ERROR: Downloaded file not found: {local_file}")
        sys.exit(1)

    file_size_mb = local_file.stat().st_size / (1024 * 1024)
    print(f"   File size: {file_size_mb:.2f} MB")

    if file_size_mb < 1:
        print("ERROR: File size is suspiciously small, download may have failed")
        sys.exit(1)

    # Upload to GCloud
    blob_path = f"vulkan/{version}/{platform}/{filename}"
    gcloud_url = f"gs://{GCLOUD_BUCKET}/{blob_path}"

    print("\nUploading to GCloud storage…")
    print(f"   Destination: {gcloud_url}")

    try:
        client = storage.Client("rerun-open")
        bucket = client.bucket(GCLOUD_BUCKET)
        blob = bucket.blob(blob_path)

        blob.upload_from_filename(str(local_file))
        print(f"OK: Successfully uploaded to {gcloud_url}")

        # Try to make the blob publicly readable
        # Note: This may fail if the bucket has uniform bucket-level access enabled
        try:
            blob.make_public()
            print("OK: Made blob publicly readable")
        except Exception:
            print("Warning: Could not set individual object ACL (uniform bucket-level access enabled)")
            print("   The bucket may already have public access configured at the bucket level")

        # Show the public URL
        public_url = f"https://storage.googleapis.com/{GCLOUD_BUCKET}/{blob_path}"
        print(f"\nPublic URL: {public_url}")

    except Exception as e:
        print(f"ERROR: Upload failed: {e}")
        print("\nMake sure you're authenticated with: gcloud auth application-default login")
        sys.exit(1)

    # Clean up local file
    print("\nCleaning up local file…")
    local_file.unlink()
    print(f"OK: Removed {local_file}")


def main() -> None:
    import argparse

    parser = argparse.ArgumentParser(
        description="Download and upload Vulkan SDK to GCloud storage",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Upload macOS SDK
  python upload_vulkan_sdk.py --platform mac

  # Upload all platforms
  python upload_vulkan_sdk.py --platform mac --platform linux --platform windows

  # Upload specific version for macOS
  python upload_vulkan_sdk.py --platform mac --version 1.3.290.0
        """,
    )

    parser.add_argument(
        "--platform",
        action="append",
        choices=["mac", "linux", "windows"],
        required=True,
        help="Platform(s) to upload (can be specified multiple times)",
    )

    parser.add_argument(
        "--version",
        default=VULKAN_VERSION,
        help=f"Vulkan SDK version to upload (default: {VULKAN_VERSION})",
    )

    args = parser.parse_args()

    print(f"Uploading Vulkan SDK version {args.version} for platforms: {', '.join(args.platform)}")

    for platform in args.platform:
        print(f"\n{'=' * 60}")
        print(f"Processing platform: {platform}")
        print(f"{'=' * 60}")
        download_and_upload_vulkan_sdk(args.version, platform)

    print(f"\n{'=' * 60}")
    print("All uploads complete!")
    print(f"{'=' * 60}")


if __name__ == "__main__":
    main()
