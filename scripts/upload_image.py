#!/usr/bin/env python3

"""
Upload an image to Google Cloud.

Installation
------------

Requires the following packages:
  pip install google-cloud-storage

Before running, you have to authenticate via the Google Cloud CLI:
- Install it (https://cloud.google.com/storage/docs/gsutil_install)
- Set up credentials (https://cloud.google.com/storage/docs/gsutil_install#authenticate)

If you get this error:

    File "…/site-packages/cryptography/hazmat/primitives/asymmetric/utils.py", line 6, in <module>
        from cryptography.hazmat.bindings._rust import asn1
    pyo3_runtime.PanicException: Python API call failed

Then run `python3 -m pip install cryptography==38.0.4`
(https://levelup.gitconnected.com/fix-attributeerror-module-lib-has-no-attribute-openssl-521a35d83769)

Usage
-----

Use the script:

    python3 scripts/upload_image.py --help

or the pixi command:

    pixi run upload-image --help

All info/debug output occurs on stderr. If stdout is not a tty (e.g. piping to `pbcopy`), the resulting HTML tag is also
printed to stdout. For example, this upload the image from the clipboard and copies the resulting HTML tag back to the
clipboard:

    pixi run upload-image --name some_name | pbcopy
"""

from __future__ import annotations

import argparse
import hashlib
import logging
import mimetypes
import os
import subprocess
import sys
import tempfile
import urllib.parse
import urllib.request
from io import BytesIO
from pathlib import Path

import PIL
import PIL.Image
import PIL.ImageGrab
import requests
import tqdm
from google.cloud import storage
from PIL.Image import Image, Resampling

# NOTE: We depend on the stack using the exact sizes they do right now in:
#       - docs codegen (`image_url_stack`)
SIZES = [
    480,
    768,
    1024,
    1200,
]


def build_image_stack(image: Image) -> list[tuple[int | None, Image]]:
    image_stack: list[tuple[int | None, Image]] = [(None, image)]

    for size in SIZES:
        if image.width > size:
            logging.info(f"Resizing to: {size}px")
            new_image = image.resize(size=(size, int(size * image.height / image.width)), resample=Resampling.LANCZOS)

            image_stack.append((size, new_image))

    return image_stack


def image_from_clipboard() -> Image | None:
    """
    Get image from the clipboard.

    On Mac, `PIL.ImageGrab.grabclipboard()` compresses to JPG. This function uses the same code but uses PNG instead.
    """
    if sys.platform == "darwin":
        fh, filepath = tempfile.mkstemp(".png")
        os.close(fh)
        commands = [
            'set theFile to (open for access POSIX file "' + filepath + '" with write permission)',
            "try",
            "    write (the clipboard as «class PNGf») to theFile",
            "end try",
            "close access theFile",
        ]
        script = ["osascript"]
        for command in commands:
            script += ["-e", command]
        subprocess.call(script)

        im = None
        if os.stat(filepath).st_size != 0:
            im = PIL.Image.open(filepath)
            im.load()
        os.unlink(filepath)
        return im
    else:
        # On windows might return a list, of files,
        # so return None signaling no image found.
        content = PIL.ImageGrab.grabclipboard()
        if isinstance(content, list):
            return None
        return content


class Uploader:
    def __init__(self) -> None:
        gcs = storage.Client("rerun-open")
        self.bucket = gcs.bucket("rerun-static-img")

    def upload_file(self, path: Path) -> str:
        """
        Upload a single file to Google Cloud.

        Parameters
        ----------
        path : Path
            The path to the file to upload.

        Returns
        -------
        str
            The name of the uploaded file.

        """

        data = path.read_bytes()
        digest = data_hash(data)
        object_name = f"{digest}_{path.name}"
        content_type, content_encoding = mimetypes.guess_type(path)

        self.upload_data(data, object_name, content_type, content_encoding)

        return object_name

    def upload_stack_from_file(self, image_path: Path, name: str | None = None) -> str:
        """
        Upload an image stack from a file.

        Parameters
        ----------
        image_path : Path
            The path to the image file.
        name : str, optional
            The name of the image stack. If None, the file name is used.

        Returns
        -------
        str
            The `<picture>` tag for the image stack.

        """
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
        """
        Upload an image stack from the clipboard.

        Parameters
        ----------
        name : str
            The name of the image stack.

        Returns
        -------
        str
            The `<picture>` tag for the image stack.

        """

        clipboard = image_from_clipboard()
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
        output_format: str | None = "PNG",
        file_ext: str | None = ".png",
        content_type: str | None = "image/png",
    ) -> str:
        """
        Create a multi-resolution stack and upload it.

        Parameters
        ----------
        image : PIL.Image.Image
            The image to upload.
        name : str
            The name of the image.
        output_format : str, optional
            The output format of the image.
        file_ext : str, optional
            The file extension of the image, including the period.
        content_type : str, optional
            The content type of the image.

        Returns
        -------
        str
            The `<picture>` HTML tag for the image stack.

        """

        logging.info(f"Base image width: {image.width}px")

        with BytesIO() as buffer:
            image.save(buffer, output_format, optimize=True, quality=80, compress_level=9)
            original_image_data = buffer.getvalue()
        digest = data_hash(original_image_data)

        image_stack = build_image_stack(image)

        html_str = "<picture>\n"

        # upload images
        for index, (width, image) in enumerate(image_stack):
            with BytesIO() as buffer:
                image.save(buffer, output_format, optimize=True, quality=80, compress_level=9)
                image_data = buffer.getvalue()

            # NOTE: We depend on the filenames using the exact format they have right now in:
            #       - docs codegen (`image_url_stack`)
            if width is None:
                object_name = f"{name}/{digest}/full{file_ext}"
            else:
                object_name = f"{name}/{digest}/{width}w{file_ext}"

            self.upload_data(image_data, object_name, content_type, None)

            if width is not None:
                html_str += (
                    f'  <source media="(max-width: {width}px)" srcset="https://static.rerun.io/{object_name}">\n'
                )
            else:
                html_str += f'  <img src="https://static.rerun.io/{object_name}" alt="">\n'

            logging.info(f"uploaded width={width or 'full'} ({index + 1}/{len(image_stack)})")

        html_str += "</picture>"
        return html_str

    def upload_data(
        self,
        data: bytes,
        path: str,
        content_type: str | None = None,
        content_encoding: str | None = None,
    ) -> None:
        """
        Low-level upload of data.

        Parameters
        ----------
        data : bytes
            The data to upload.
        path : str
            The path of the object.
        content_type : str, optional
            The content type of the object.
        content_encoding : str, optional
            The content encoding of the object.

        """

        logging.info(f"Uploading {path} (size: {len(data)}, type: {content_type}, encoding: {content_encoding})")
        destination = self.bucket.blob(path)
        destination.content_type = content_type
        destination.content_encoding = content_encoding

        if destination.exists():
            logging.warning(f"blob {path} already exists in GCS, skipping upload")
            return

        stream = BytesIO(data)
        destination.upload_from_file(stream)


def data_hash(data: bytes) -> str:
    """Compute a sha1 hash digest of some data."""
    return hashlib.sha1(data).hexdigest()


def download_file(url: str, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    logging.info("Downloading %s to %s", url, path)
    response = requests.get(url, stream=True)
    with tqdm.tqdm.wrapattr(
        open(path, "wb"),
        "write",
        miniters=1,
        total=int(response.headers.get("content-length", 0)),
        desc=f"Downloading {path.name}",
    ) as f:
        for chunk in response.iter_content(chunk_size=4096):
            f.write(chunk)


def run(args: argparse.Namespace) -> None:
    """Run the script based on the provided args."""
    try:
        uploader = Uploader()

        if args.single:
            if args.path is None:
                raise RuntimeError("Path is required when uploading a single image")

            object_name = uploader.upload_file(args.path)
            html_str = f"https://static.rerun.io/{object_name}"
        elif args.path is None:
            if args.name is None:
                raise RuntimeError("Name is required when uploading from clipboard")
            else:
                html_str = uploader.upload_stack_from_clipboard(args.name)
        else:
            html_str = uploader.upload_stack_from_file(args.path, args.name)

    except RuntimeError as e:
        print(f"Error: {e.args[0]}", file=sys.stderr)
        return

    print(f"\n{html_str}", file=sys.stderr)

    if not sys.stdout.isatty():
        # we might be piping to pbcopy or similar, so we print string again to stdout
        print(html_str)


DESCRIPTION = """Upload an image to static.rerun.io.

Example screenshots
-------------------

To make example screenshots, follow these steps:
1. Run the example.
2. Resize the Rerun window to an approximate 16:9 aspect ratio and a width of ~1500px.
   Note: you will get a warning and a confirmation prompt if the aspect ratio is not within ~10% of 16:9.
3. Groom the blueprints and panel visibility to your liking.
4. Take a screenshot using the command palette.
5. Run: pixi run upload-image --name <name_of_example>
6. Copy the output HTML tag and paste it into the README.md file.

Other uses
----------

Download an image, optimize it and create a multi-resolution stack:

    pixi run upload-image --name <name_of_stack> https://example.com/path/to/image.png
"""


def main() -> None:
    parser = argparse.ArgumentParser(description=DESCRIPTION, formatter_class=argparse.RawTextHelpFormatter)
    parser.add_argument(
        "path",
        type=str,
        nargs="?",
        help="Image file URL or path. If not provided, use the clipboard's content.",
    )
    parser.add_argument(
        "--single",
        action="store_true",
        help="Upload a single image instead of creating a multi-resolution stack.",
    )
    parser.add_argument("--name", type=str, help="Image name (required when uploading from clipboard).")
    parser.add_argument("--debug", action="store_true", help="Enable debug logging.")
    args = parser.parse_args()

    if args.debug:
        logging.basicConfig(level=logging.DEBUG)
    else:
        logging.basicConfig(level=logging.INFO)

    # The entire block is wrapped around tmp_dir such that it exists for the entire run.
    with tempfile.TemporaryDirectory() as tmp_dir:
        # check if path as a URL and download it.
        if args.path is not None:
            res = urllib.parse.urlparse(args.path)
            if res.scheme and res.netloc:
                file_name = os.path.basename(res.path)
                local_path = Path(tmp_dir) / file_name
                download_file(args.path, local_path)
                args.path = Path(local_path)
            else:
                args.path = Path(args.path)

        run(args)


if __name__ == "__main__":
    main()
