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

from google.cloud.storage import Bucket
from google.cloud.storage import Client as Gcs


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
    if arch == "x86_64" or arch == "aarch64":
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


def detect_pixi() -> bool:
    path = os.environ.get("PATH")
    return path is not None and ".pixi/env/bin" in path


class BuildMode(Enum):
    PYPI = "pypi"
    PR = "pr"

    def __str__(self) -> str:
        return self.value


def build_and_upload(bucket: Bucket, mode: BuildMode, gcs_dir: str, target: str, compatibility: str) -> None:
    if detect_pixi():
        raise Exception("the build script cannot be started in the pixi environment")

    if mode is BuildMode.PYPI:
        # Only build web viewer when publishing to pypi
        run("pixi run cargo run --locked -p re_build_web_viewer -- --release")
        maturin_feature_flags = "--no-default-features --features pypi"
    elif mode is BuildMode.PR:
        maturin_feature_flags = "--no-default-features --features extension-module"

    dist = f"dist/{target}"

    compatibility = f"--compatibility {compatibility}" if compatibility is not None else ""

    # Build into `dist`
    run(
        "maturin build "
        f"{compatibility} "
        "--manifest-path rerun_py/Cargo.toml "
        "--release "
        f"--target {target} "
        f"{maturin_feature_flags} "
        f"--out {dist}",
        env={**os.environ.copy(), "RERUN_IS_PUBLISHING": "yes"},  # stop `re_web_viewer` from building here
    )

    pkg = os.listdir(dist)[0]

    # Upload to GCS
    print("Uploading to GCS…")
    bucket.blob(f"{gcs_dir}/{pkg}").upload_from_filename(f"{dist}/{pkg}")


def main() -> None:
    parser = argparse.ArgumentParser(description="Build and upload wheels to GCS")
    parser.add_argument(
        "--mode", default=BuildMode.PR, type=BuildMode, choices=list(BuildMode), help="What to build for"
    )
    parser.add_argument("--dir", required=True, help="Upload the wheel to the given directory in GCS")
    parser.add_argument("--target", help="Target to build for")
    parser.add_argument(
        "--compat",
        type=str,
        help='The platform tag for linux, e.g. "manylinux_2_31"',
    )
    args = parser.parse_args()

    build_and_upload(
        Gcs("rerun-open").bucket("rerun-builds"),
        args.mode,
        args.dir,
        args.target or detect_target(),
        args.compat,
    )


if __name__ == "__main__":
    main()
