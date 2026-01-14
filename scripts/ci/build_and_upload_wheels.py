#!/usr/bin/env python3

"""
Build and upload wheels to GCS.

Install dependencies:
    python3 -m pip install google-cloud-storage==2.9.0

Use the script:
    python3 scripts/ci/release.py --help
"""

from __future__ import annotations

import argparse
import os
import platform
import subprocess
from enum import Enum

from google.cloud.storage import Bucket, Client as Gcs


def run(
    cmd: str,
    *,
    cwd: str | None = None,
    env: dict[str, str] | None = None,
) -> None:
    print(f"{cwd or ''}> {cmd}")
    subprocess.check_output(cmd.split(), cwd=cwd, env=env)


def detect_target() -> str:
    arch = platform.machine()
    if arch in {"x86_64", "aarch64"}:
        pass  # leave it as is
    elif arch == "arm64":
        arch = "aarch64"
    else:
        raise Exception(f"unknown architecture: {arch}")

    os = platform.system()
    if os == "Linux":
        os = "unknown-linux-gnu"
    elif os == "Darwin":
        os = "apple-darwin"
    elif os == "Windows":
        os = "pc-windows-msvc"
    else:
        raise Exception(f"unknown platform: {os}")

    return f"{arch}-{os}"


class BuildMode(Enum):
    PYPI = "pypi"
    PR = "pr"
    EXTRA = "extra"

    def __str__(self) -> str:
        return self.value


def build_and_upload(
    bucket: Bucket | None, mode: BuildMode, gcs_dir: str, target: str, compatibility: str | None
) -> None:
    # pypi / extra builds require a web build
    if mode in (BuildMode.PYPI, BuildMode.EXTRA):
        run("pixi run rerun-build-web-release")

    if mode is BuildMode.PYPI:
        maturin_feature_flags = "--no-default-features --features pypi"
    elif mode is BuildMode.PR:
        maturin_feature_flags = "--no-default-features --features extension-module"
    elif mode is BuildMode.EXTRA:
        maturin_feature_flags = "--no-default-features --features pypi,extra"

    dist = f"dist/{target}"

    compatibility = f"--compatibility {compatibility}" if compatibility is not None else ""

    run(
        "maturin build "
        f"{compatibility} "
        "--manifest-path rerun_py/Cargo.toml "
        "--release "
        f"--target {target} "
        f"{maturin_feature_flags} "
        f"--out {dist}",
    )

    pkg = os.listdir(dist)[0]

    if bucket is not None:
        # Upload to GCS
        print("Uploading to GCS…")
        bucket.blob(f"{gcs_dir}/{pkg}").upload_from_filename(f"{dist}/{pkg}")


def main() -> None:
    parser = argparse.ArgumentParser(description="Build and upload wheels to GCS")
    parser.add_argument(
        "--mode",
        default=BuildMode.PR,
        type=BuildMode,
        choices=list(BuildMode),
        help="What to build for",
    )
    parser.add_argument("--dir", required=True, help="Upload the wheel to the given directory in GCS")
    parser.add_argument("--target", help="Target to build for")
    parser.add_argument(
        "--compat",
        type=str,
        help='The platform tag for linux, e.g. "manylinux_2_28"',
    )
    parser.add_argument("--upload-gcs", action="store_true", default=False, help="Upload the wheel to GCS")
    args = parser.parse_args()

    if args.upload_gcs:
        bucket = Gcs("rerun-open").bucket("rerun-builds")
    else:
        bucket = None

    build_and_upload(
        bucket,
        args.mode,
        args.dir,
        args.target or detect_target(),
        args.compat,
    )


if __name__ == "__main__":
    main()
