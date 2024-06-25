#!/usr/bin/env python3

"""
Build and upload rerun_notebook wheels to GCS.

IMPORTANT: rerun_js must be built beforehand, otherwise this script will fail. Use `pixi run js-build-base`.
"""

from __future__ import annotations

import argparse
import os
import subprocess

from google.cloud.storage import Bucket, Client as Gcs


def run(
    cmd: str,
    *,
    cwd: str | None = None,
    env: dict[str, str] | None = None,
) -> None:
    print(f"{cwd or ''}> {cmd}")
    subprocess.check_output(cmd.split(), cwd=cwd, env=env)


def build_and_upload(bucket: Bucket, gcs_dir: str) -> None:
    dist = "dist/"

    # Build into `dist`
    run(
        f"hatch build -t wheel ../{dist}",
        cwd="rerun_notebook",
    )

    pkg = os.listdir(dist)[0]

    # Upload to GCS
    print("Uploading to GCS…")
    bucket.blob(f"{gcs_dir}/{pkg}").upload_from_filename(f"{dist}/{pkg}")


def main() -> None:
    parser = argparse.ArgumentParser(description="Build and upload rerun_notebook wheels to GCS")
    parser.add_argument("--dir", required=True, help="Upload the wheel to the given directory in GCS")
    args = parser.parse_args()

    build_and_upload(
        Gcs("rerun-open").bucket("rerun-builds"),
        args.dir,
    )


if __name__ == "__main__":
    main()
