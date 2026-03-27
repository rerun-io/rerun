#!/usr/bin/env python3

"""
Build and upload rerun_notebook wheels to GCS.

IMPORTANT: rerun_js must be built beforehand, otherwise this script will fail. Use `pixi run js-build-base`.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import zipfile

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


def build_and_upload(bucket: Bucket, gcs_dir: str) -> str:
    dist = "dist/"

    # Build into `dist`
    run(
        f"hatch build -t wheel ../{dist}",
        cwd="rerun_notebook",
    )

    pkg = os.listdir(dist)[0]
    wheel = f"{dist}/{pkg}"

    # Upload to GCS
    print("Uploading to GCS…")
    bucket.blob(f"{gcs_dir}/{pkg}").upload_from_filename(wheel)

    return wheel


def publish_notebook_asset(bucket: Bucket, gcs_dir: str, wheel: str) -> None:
    """Extract widget.js and re_viewer_bg.wasm from the notebook wheel and upload to the web viewer bucket."""

    with zipfile.ZipFile(wheel, "r") as archive:
        archive.extract("rerun_notebook/static/widget.js", "extracted")
        archive.extract("rerun_notebook/static/re_viewer_bg.wasm", "extracted")

    for filename in ["widget.js", "re_viewer_bg.wasm"]:
        local_path = f"extracted/rerun_notebook/static/{filename}"
        blob = bucket.blob(f"{gcs_dir}/{filename}")
        print(f"Uploading {local_path} to gs://{bucket.name}/{blob.name}")
        blob.upload_from_filename(local_path)


def main() -> None:
    parser = argparse.ArgumentParser(description="Build and upload rerun_notebook wheels to GCS")
    parser.add_argument("--dir", required=True, help="Upload the wheel to the given directory in GCS")
    parser.add_argument(
        "--notebook-dir",
        required=False,
        help="Upload notebook assets (widget.js, re_viewer_bg.wasm) to the given directory in the web viewer bucket",
    )
    args = parser.parse_args()

    gcs = Gcs("rerun-open")

    wheel = build_and_upload(
        gcs.bucket("rerun-builds"),
        args.dir,
    )

    if args.notebook_dir:
        publish_notebook_asset(
            gcs.bucket("rerun-web-viewer"),
            args.notebook_dir,
            wheel,
        )


if __name__ == "__main__":
    main()
