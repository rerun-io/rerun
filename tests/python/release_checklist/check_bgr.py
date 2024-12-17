from __future__ import annotations

import os
from argparse import Namespace
from io import BytesIO
from uuid import uuid4

import numpy as np
import requests
import rerun as rr
import rerun.blueprint as rrb
from PIL import Image

README = """\
# BGR Support

This checks whether BGR images with various datatypes are supported.

### Action
All images should look the same (and sane).

"""

types = [
    # Skipping on i8, since it would look different.
    ("u8", np.uint8),
    ("u16", np.uint16),
    ("u32", np.uint32),
    ("u64", np.uint64),
    ("i16", np.int16),
    ("i32", np.int32),
    ("i64", np.int64),
    ("f16", np.float16),
    ("f32", np.float32),
    ("f64", np.float64),
]


def blueprint() -> rrb.BlueprintLike:
    entities = [f"bgr_{type}" for (type, _) in types] + [f"bgra_{type}" for (type, _) in types] + ["rgb_u8"]
    return rrb.Grid(
        rrb.Grid(contents=[rrb.Spatial2DView(origin=path) for path in entities]),
        rrb.TextDocumentView(origin="readme", name="Instructions"),
    )


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), timeless=True)


def run_bgr_images(sample_image_rgb_u8: np.ndarray) -> None:
    # We're being explicit about datatypes & datamodels on all calls to avoid confunsion.

    # Show the original image as a reference:
    rr.log("rgb_u8", rr.Image(sample_image_rgb_u8, color_model="RGB", datatype="u8"))

    sample_image_bgr_u8 = sample_image_rgb_u8[:, :, ::-1]
    sample_image_bgra_u8 = np.insert(sample_image_bgr_u8, 3, 255, axis=2)

    for datatype, dtype in types:
        sample_image_bgr = np.asarray(sample_image_bgr_u8, dtype=dtype)
        rr.log(f"bgr_{datatype}", rr.Image(sample_image_bgr, color_model="BGR", datatype=datatype))
        sample_image_bgra = np.asarray(sample_image_bgra_u8, dtype=dtype)
        rr.log(f"bgra_{datatype}", rr.Image(sample_image_bgra, color_model="BGRA", datatype=datatype))


def download_example_image_as_rgb() -> np.ndarray:
    # Download this recreation of the lena image (via https://mortenhannemose.github.io/lena/):
    # https://mortenhannemose.github.io/assets/img/Lena_512.png
    url = "https://mortenhannemose.github.io/assets/img/Lena_512.png"
    response = requests.get(url)
    image = Image.open(BytesIO(response.content))
    image = image.convert("RGB")
    return np.array(image)


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())
    rr.send_blueprint(blueprint(), make_active=True, make_default=True)

    sample_image_rgb_u8 = download_example_image_as_rgb()
    log_readme()
    run_bgr_images(sample_image_rgb_u8)


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
