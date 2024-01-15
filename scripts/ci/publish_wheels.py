#!/usr/bin/env python3

"""
Publish wheels to PyPI.

Install dependencies:
    python3 -m pip install packaging==23.1 google-cloud-storage==2.9.0

Use the script:
    python3 scripts/ci/verify_wheels.py --help
"""
from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import wheel_utils
from google.cloud.storage import Blob
from google.cloud.storage import Client as Gcs
from packaging.utils import canonicalize_version


def run(
    cmd: str,
    *,
    cwd: str | None = None,
    env: dict[str, str] | None = None,
) -> None:
    print(f"{cwd or ''}> {cmd}")
    subprocess.check_output(cmd.split(), cwd=cwd, env=env)


def check_version(expected_version: str) -> None:
    wheels = list(Path("wheels").glob("*.whl"))

    for wheel in wheels:
        wheel_version = wheel.stem.split("-")[1]
        if canonicalize_version(wheel_version) != expected_version:
            print(f"Unexpected version: {wheel_version} (expected: {expected_version}) in {wheel.name}")
            sys.exit(1)

    print(f"All wheel versions match the expected version: {expected_version}")


def main() -> None:
    parser = argparse.ArgumentParser(description="Publish wheels to PyPI")
    parser.add_argument("--version", required=True, help="Version to expect")
    parser.add_argument("--dir", required=True, help="Directory in GCS to fetch wheels from")
    parser.add_argument("--repository", required=True, help="PyPI repository")
    parser.add_argument("--token", required=True, help="PyPI token")
    args = parser.parse_args()

    bucket = Gcs("rerun-open").bucket("rerun-builds")
    wheel_blobs: list[Blob] = list(blob for blob in bucket.list_blobs(prefix=args.dir) if blob.name.endswith(".whl"))
    wheels = [blob.name.split("/")[-1] for blob in wheel_blobs]
    wheel_paths = [f"wheels/{wheel}" for wheel in wheels]
    wheel_utils.check_expected_wheels(wheels)

    if os.path.exists("wheels"):
        shutil.rmtree("wheels")
    os.mkdir("wheels")
    with ThreadPoolExecutor() as e:
        for blob in wheel_blobs:
            e.submit(lambda: blob.download_to_filename(f"wheels/{blob.name.split('/')[-1]}"))

    check_version(canonicalize_version(args.version))

    run(
        f"maturin upload --skip-existing {' '.join(wheel_paths)}",
        env={
            **os.environ,
            "MATURIN_REPOSITORY": args.repository,
            "MATURIN_PYPI_TOKEN": args.token,
        },
    )


if __name__ == "__main__":
    main()
