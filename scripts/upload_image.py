#!/usr/bin/env python3

"""
Upload an image to Google Cloud.

Requires the following packages:
  pip install google-cloud-storage # NOLINT

Before running, you have to authenticate via the Google Cloud CLI:
- Install it (https://cloud.google.com/storage/docs/gsutil_install)
- Set up credentials (https://cloud.google.com/storage/docs/gsutil_install#authenticate)

If you get this error:

    File "…/site-packages/cryptography/hazmat/primitives/asymmetric/utils.py", line 6, in <module>
        from cryptography.hazmat.bindings._rust import asn1
    pyo3_runtime.PanicException: Python API call failed

Then run `python3 -m pip install cryptography==38.0.4`
(https://levelup.gitconnected.com/fix-attributeerror-module-lib-has-no-attribute-openssl-521a35d83769)
"""

from __future__ import annotations

import argparse
import hashlib
import logging
import mimetypes
import sys
from io import BytesIO
from pathlib import Path
from typing import BinaryIO

import PIL
import PIL.Image
import PIL.ImageGrab
from google.cloud import storage
from PIL.Image import Image, Resampling

SIZES = [
    480,
    768,
    1024,
    1200,
]


class Uploader:
    def __init__(self):
        gcs = storage.Client("rerun-open")
        self.bucket = gcs.bucket("rerun-static-img")

    def upload_file(self, path: Path) -> str:
        path = path.resolve()
        digest = content_hash(path)

        object_name = f"{digest}_{path.name}"
        content_type, content_encoding = mimetypes.guess_type(path)
        with open(path, "rb") as f:
            self.upload_data(f, object_name, content_type, content_encoding)

        return object_name

    def upload_stack_from_file(self, image_path: Path, name: str | None = None) -> str:
        image = PIL.Image.open(image_path)
        content_type, _ = mimetypes.guess_type(image_path)

        return self.upload_stack(
            image,
            name=name if name is not None else image_path.stem,
            output_format=image.format,
            file_ext=image_path.suffix,
            content_type=content_type,
        )

    def upload_stack_from_clipboard(self, name: str) -> str:
        clipboard = PIL.ImageGrab.grabclipboard()
        if isinstance(clipboard, PIL.Image.Image):
            image = clipboard
            return self.upload_stack(
                image,
                name=name,
            )
        else:
            raise RuntimeError("No image found on clipboard")

    def upload_stack(
        self,
        image: Image,
        name: str,
        output_format: str = "PNG",
        file_ext: str = ".png",
        content_type: str = "image/png",
    ) -> str:
        """Create a multi-resolution stack and upload it."""

        logging.info(f"Base image width: {image.width}px")

        # build image stack
        image_stack: list[tuple[str, int | None, Image]] = []
        for width in SIZES:
            if image.width > width:
                logging.info(f"Resizing to: {width}px")
                new_image = image.resize(
                    size=(width, int(width * image.height / image.width)), resample=Resampling.LANCZOS
                )

                image_stack.append((f"{name}_{width}w", width, new_image))

        image_stack.append((f"{name}_full", None, image))

        html_str = "<picture>\n"

        # upload images
        for name, width, image in image_stack:
            with BytesIO() as buffer:
                image.save(buffer, output_format)
                buffer.seek(0)
                digest = content_hash(buffer)
                buffer.seek(0)

                object_name = f"{digest}_{name}{file_ext}"
                logging.info(f"Uploading image: {object_name} (size: {buffer.getbuffer().nbytes} bytes)")
                self.upload_data(buffer, object_name, content_type, None)

                if width is not None:
                    html_str += (
                        f'  <source media="(max-width: {width}px)" srcset="https://static.rerun.io/{object_name}">\n'
                    )
                else:
                    html_str += f'  <img src="https://static.rerun.io/{object_name}" alt="">\n'

        html_str += "</picture>"
        return html_str

    def upload_data(
        self, data: BinaryIO, name: str, content_type: str | None = None, content_encoding: str | None = None
    ):
        """Low-level upload of data."""

        logging.debug(f"Uploading {name} (type: {content_type}, encoding: {content_encoding})")
        destination = self.bucket.blob(name)
        destination.content_type = content_type
        destination.content_encoding = content_encoding
        destination.upload_from_file(data)


def content_hash(data: Path | BinaryIO) -> str:
    h = hashlib.sha1()
    b = bytearray(128 * 1024)
    mv = memoryview(b)

    def update(stream: BinaryIO) -> None:
        while n := stream.readinto(mv):
            h.update(mv[:n])

    if isinstance(data, Path):
        with open(data, "rb", buffering=0) as f:
            update(f)
    else:
        update(data)

    return h.hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(description="Upload an image.")
    parser.add_argument(
        "path", type=Path, nargs="?", help="File path to the image. If not provided, use the clipboard's content."
    )
    parser.add_argument(
        "--single", action="store_true", help="Upload a single image instead of creating a multi-resolution stack."
    )
    parser.add_argument("--name", type=str, help="Image name (required when uploading from clipboard).")
    parser.add_argument("--debug", action="store_true", help="Enable debug logging.")
    args = parser.parse_args()

    if args.debug:
        logging.basicConfig(level=logging.DEBUG)
    else:
        logging.basicConfig(level=logging.INFO)

    uploader = Uploader()

    try:
        if args.single:
            object_name = uploader.upload_file(args.path)
            print(f"https://static.rerun.io/{object_name}")
        else:
            if args.path is None:
                if args.name is None:
                    raise RuntimeError("Name is required when uploading from clipboard")
                else:
                    html_str = uploader.upload_stack_from_clipboard(args.name)
            else:
                html_str = uploader.upload_stack_from_file(args.path, args.name)
            print("\n" + html_str)
    except RuntimeError as e:
        print(f"Error: {e.args[0]}", file=sys.stderr)


if __name__ == "__main__":
    main()
