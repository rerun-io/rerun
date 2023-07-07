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

or the just command:

    just upload --help
"""

from __future__ import annotations

import argparse
import hashlib
import logging
import mimetypes
import os
import shutil
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

SIZES = [
    480,
    768,
    1024,
    1200,
]


def image_from_clipboard() -> Image:
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
        return PIL.ImageGrab.grabclipboard()


class Uploader:
    def __init__(self, pngcrush: bool):
        gcs = storage.Client("rerun-open")
        self.bucket = gcs.bucket("rerun-static-img")
        self.run_pngcrush = pngcrush

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

        image_data = path.read_bytes()
        digest = data_hash(image_data)
        object_name = f"{digest}_{path.name}"
        content_type, content_encoding = mimetypes.guess_type(path)

        self.upload_data(image_data, object_name, content_type, content_encoding)

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
        output_format: str = "PNG",
        file_ext: str = ".png",
        content_type: str = "image/png",
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
            The file extension of the image.
        content_type : str, optional
            The content type of the image.

        Returns
        -------
        str
            The `<picture>` HTML tag for the image stack.
        """

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
                image.save(buffer, output_format, optimize=True, quality=80, compress_level=9)
                image_data = buffer.getvalue()

            digest = data_hash(image_data)

            object_name = f"{digest}_{name}{file_ext}"
            self.upload_data(image_data, object_name, content_type, None)

            if width is not None:
                html_str += (
                    f'  <source media="(max-width: {width}px)" srcset="https://static.rerun.io/{object_name}">\n'
                )
            else:
                html_str += f'  <img src="https://static.rerun.io/{object_name}" alt="">\n'

        html_str += "</picture>"
        return html_str

    def upload_data(self, data: bytes, name: str, content_type: str | None = None, content_encoding: str | None = None):
        """
        Low-level upload of data.

        Parameters
        ----------
        data : bytes
            The data to upload.
        name : str
            The name of the object.
        content_type : str, optional
            The content type of the object.
        content_encoding : str, optional
            The content encoding of the object.
        """

        if self.run_pngcrush and content_type == "image/png":
            data = run_pngcrush(data)

        logging.info(f"Uploading {name} (size: {len(data)}, type: {content_type}, encoding: {content_encoding})")
        destination = self.bucket.blob(name)
        destination.content_type = content_type
        destination.content_encoding = content_encoding

        stream = BytesIO(data)
        destination.upload_from_file(stream)


def run_pngcrush(data: bytes) -> bytes:
    """
    Run pngcrush on some data.

    Parameters
    ----------
    data : bytes
        The PNG data to crush.

    Returns
    -------
    bytes
        The crushed PNG data.
    """

    with tempfile.TemporaryDirectory() as tmpdir:
        input_file = Path(tmpdir) / "input.png"
        input_file.write_bytes(data)

        output_file = Path(tmpdir) / "output.png"
        os.system(f"pngcrush -q -warn -rem allb -reduce {input_file} {output_file}")
        output_data = output_file.read_bytes()

    input_len = len(data)
    output_len = len(output_data)
    if output_len > input_len:
        logging.info("pngcrush failed to reduce file size")
        return data
    else:
        logging.info(
            f"pngcrush reduced size from {input_len} to {output_len} bytes "
            f"({(input_len - output_len) *100/ input_len:.2f}%)"
        )
        return output_data


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


def run(args) -> None:
    """Run the script based on the provided args."""
    try:
        if shutil.which("pngcrush") is None and not args.skip_pngcrush:
            raise RuntimeError("pngcrush is not installed, consider using --skip-pngcrush")

        uploader = Uploader(not args.skip_pngcrush)

        if args.single:
            object_name = uploader.upload_file(args.path)
            print(f"\nhttps://static.rerun.io/{object_name}")
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


DESCRIPTION = """Upload an image to static.rerun.io.

Example screenshots
-------------------

To make example screenshots, follow these steps:
1. Run the example.
2. Resize the Rerun window to an approximate 16:9 aspect ratio and a width of ~1500px.
3. Groom the blueprints and panel visibility to your liking.
4. Take a screenshot using the command palette.
5. Run: just upload --name <name_of_example>
6. Copy the output HTML tag and paste it into the README.md file.

Other uses
----------

Download an image, optimize it and create a multi-resolution stack:

    just upload --name <name_of_stack> https://example.com/path/to/image.png
"""


def main() -> None:
    parser = argparse.ArgumentParser(description=DESCRIPTION, formatter_class=argparse.RawTextHelpFormatter)
    parser.add_argument(
        "path", type=str, nargs="?", help="Image file URL or path. If not provided, use the clipboard's content."
    )
    parser.add_argument(
        "--single", action="store_true", help="Upload a single image instead of creating a multi-resolution stack."
    )
    parser.add_argument("--name", type=str, help="Image name (required when uploading from clipboard).")
    parser.add_argument("--skip-pngcrush", action="store_true", help="Skip PNGCrush.")
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
