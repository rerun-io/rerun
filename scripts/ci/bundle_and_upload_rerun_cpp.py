#!/usr/bin/env python3

from __future__ import annotations

import argparse
import hashlib
import logging
import multiprocessing
import os
import shutil
import subprocess
import tempfile

from google.cloud import storage


def download_rerun_c(target_dir: str, git_hash: str, platform_filter: str = None) -> None:
    os.mkdir(target_dir)

    # See reusable_build_and_upload_rerun_c.yml for available libraries.
    # See rerun_cpp/CMakeLists.txt for library names.
    for src, dst in [
        ("linux/librerun_c.a", "librerun_c__linux_x64.a"),
        ("macos-arm/librerun_c.a", "librerun_c__macos_arm64.a"),
        ("macos-intel/librerun_c.a", "librerun_c__macos_x64.a"),
        ("windows/librerun_c.a", "librerun_c__win_x64.a"),
    ]:
        if platform_filter is not None and src.startswith(platform_filter) is False:
            continue

        url = f"https://build.rerun.io/commit/{git_hash}/rerun_c/{src}"
        logging.info(f"Downloading rerun_c from {url}")

        if os.system(f"curl -L -o {target_dir}/{dst} {url}") != 0:
            raise RuntimeError(f"Failed to download {url}")


def package_rerun_cpp(root_dir: str, base_dir: str) -> str:
    logging.info(f"Packaging {base_dir}.zip...")

    rerun_zip = shutil.make_archive(root_dir + "/" + base_dir, "zip", root_dir=root_dir, base_dir=base_dir)
    with open(rerun_zip, "rb", buffering=0) as f:
        sha256 = hashlib.file_digest(f, "sha256").hexdigest()
        print(f"SHA256={sha256}")
    return rerun_zip


def upload_rerun_cpp(rerun_zip: str, git_hash: str) -> None:
    logging.info("Uploading to gcloud...")

    gcs = storage.Client("rerun-open")
    bucket = gcs.bucket("rerun-builds")
    destination = bucket.blob(f"commit/{git_hash}/rerun_cpp.zip")
    destination.content_type = "application/zip"
    destination.upload_from_filename(rerun_zip)
    logging.info(f"Uploaded to https://build.rerun.io/commit/{git_hash}/rerun_cpp.zip")


def test_rerun_cpp(git_hash: str) -> None:
    logging.info("Testing uploaded artifact...")

    with tempfile.TemporaryDirectory() as testdir:
        shutil.copytree("examples/cpp/minimal/", testdir, dirs_exist_ok=True)

        args = ["cmake", f"-DRERUN_CPP_URL=https://build.rerun.io/commit/{git_hash}/rerun_cpp.zip", "."]
        logging.info(f"> {subprocess.list2cmdline(args)}")
        result = subprocess.run(args, cwd=testdir, check=False, capture_output=True, text=True)
        assert (
            result.returncode == 0
        ), f"{subprocess.list2cmdline(args)} failed with exit-code {result.returncode}. Output:\n{result.stdout}\n{result.stderr}"

        args = ["cmake", "--build", ".", "--parallel", str(multiprocessing.cpu_count())]
        logging.info(f"> {subprocess.list2cmdline(args)}")
        result = subprocess.run(args, cwd=testdir, check=False, capture_output=True, text=True)
        assert (
            result.returncode == 0
        ), f"{subprocess.list2cmdline(args)} failed with exit-code {result.returncode}. Output:\n{result.stdout}\n{result.stderr}"


def main() -> None:
    logging.basicConfig(level=logging.INFO)

    parser = argparse.ArgumentParser(
        description="Bundle and upload rerun_cpp sdk. Assumes rerun_c already built & uploaded."
    )
    parser.add_argument(
        "--git-hash",
        required=True,
        type=str,
        help="Git hash for which we're downloading rerun_c and uploading rerun_cpp.",
    )
    parser.add_argument(
        "--platform-filter",
        type=str,
        default=None,
        help="If set, only the specified platform will be fetched for rerun_c.",
    )
    parser.add_argument("--no-upload", help="If true, don't upload rerun_cpp.", action="store_true")
    parser.add_argument("--skip-test", help="If true, don't test rerun_cpp after upload.", action="store_true")
    parser.add_argument(
        "--local-path", required=False, default=None, type=str, help="If set, rerun_cpp bundle will be written on disk."
    )
    args = parser.parse_args()

    git_hash = args.git_hash[:7]

    with tempfile.TemporaryDirectory() as scratch_dir:
        package_name = "rerun_cpp"
        package_dir = scratch_dir + "/" + package_name
        os.mkdir(package_dir)

        download_rerun_c(package_dir + "/lib", git_hash, args.platform_filter)
        shutil.copy("rerun_cpp/CMakeLists.txt", package_dir + "/CMakeLists.txt")
        shutil.copytree("rerun_cpp/src/", package_dir + "/src/")

        rerun_zip = package_rerun_cpp(scratch_dir, package_name)

        if args.local_path is not None:
            logging.info(f"Copying rerun_cpp bundle to local path from '{rerun_zip}' to '{args.local_path}'")
            shutil.copy(rerun_zip, args.local_path)

        if args.no_upload is not True:
            upload_rerun_cpp(rerun_zip, git_hash)

    if args.skip_test is not True and args.no_upload is not True:
        test_rerun_cpp(git_hash)


if __name__ == "__main__":
    main()
