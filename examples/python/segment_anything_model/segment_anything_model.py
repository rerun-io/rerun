#!/usr/bin/env python3
"""
Example of using Rerun to log and visualize the output of [Segment Anything](https://github.com/facebookresearch/segment-anything).

Can be used to test mask-generation on one or more images. Images can be local file-paths
or remote urls.
"""

from __future__ import annotations

import argparse
import logging
import os
from pathlib import Path
from typing import TYPE_CHECKING, Final
from urllib.parse import urlparse

import cv2
import numpy as np
import requests
import rerun as rr  # pip install rerun-sdk
import rerun.blueprint as rrb
import torch
import torchvision
from segment_anything import SamAutomaticMaskGenerator, sam_model_registry
from tqdm import tqdm

if TYPE_CHECKING:
    from segment_anything.modeling import Sam

DESCRIPTION = """
Example of using Rerun to log and visualize the output of [Segment Anything](https://github.com/facebookresearch/segment-anything).

The full source code for this example is available [on GitHub](https://github.com/rerun-io/rerun/blob/latest/examples/python/segment_anything_model).
""".strip()

MODEL_DIR: Final = Path(os.path.dirname(__file__)) / "model"
MODEL_URLS: Final = {
    "vit_h": "https://dl.fbaipublicfiles.com/segment_anything/sam_vit_h_4b8939.pth",
    "vit_l": "https://dl.fbaipublicfiles.com/segment_anything/sam_vit_l_0b3195.pth",
    "vit_b": "https://dl.fbaipublicfiles.com/segment_anything/sam_vit_b_01ec64.pth",
}


def download_with_progress(url: str, dest: Path) -> None:
    """Download file with tqdm progress bar."""
    chunk_size = 1024 * 1024
    resp = requests.get(url, stream=True)
    total_size = int(resp.headers.get("content-length", 0))
    with open(dest, "wb") as dest_file:
        with tqdm(
            desc="Downloading model",
            total=total_size,
            unit="iB",
            unit_scale=True,
            unit_divisor=1024,
        ) as progress:
            for data in resp.iter_content(chunk_size):
                dest_file.write(data)
                progress.update(len(data))


def get_downloaded_model_path(model_name: str) -> Path:
    """Fetch the segment-anything model to a local cache directory."""
    model_url = MODEL_URLS[model_name]

    model_location = MODEL_DIR / model_url.split("/")[-1]
    if not model_location.exists():
        os.makedirs(MODEL_DIR, exist_ok=True)
        download_with_progress(model_url, model_location)

    return model_location


def create_sam(model: str, device: str) -> Sam:
    """Load the segment-anything model, fetching the model-file as necessary."""
    model_path = get_downloaded_model_path(model)

    logging.info(f"PyTorch version: {torch.__version__}")
    logging.info(f"Torchvision version: {torchvision.__version__}")
    logging.info(f"CUDA is available: {torch.cuda.is_available()}")

    logging.info(f"Building sam from: {model_path}")
    sam = sam_model_registry[model](checkpoint=model_path)
    return sam.to(device=device)


def run_segmentation(mask_generator: SamAutomaticMaskGenerator, image: cv2.typing.MatLike) -> None:
    """Run segmentation on a single image."""
    rr.log("image", rr.Image(image))

    logging.info("Finding masks")
    masks = mask_generator.generate(image)

    logging.info(f"Found {len(masks)} masks")

    # Log all the masks stacked together as a tensor
    # TODO(jleibs): Tensors with class-ids and annotation-coloring would make this much slicker
    mask_tensor = (
        np.dstack([np.zeros((image.shape[0], image.shape[1]))] + [m["segmentation"] for m in masks]).astype("uint8")
        * 128
    )
    rr.log("mask_tensor", rr.Tensor(mask_tensor))

    # Note: for stacking, it is important to sort these masks by area from largest to smallest
    # this is because the masks are overlapping and we want smaller masks to
    # be drawn on top of larger masks.
    # TODO(jleibs): we could instead draw each mask as a separate image layer, but the current layer-stacking
    # does not produce great results.
    masks_with_ids = list(enumerate(masks, start=1))
    masks_with_ids.sort(key=(lambda x: x[1]["area"]), reverse=True)

    # Layer all of the masks together, using the id as class-id in the segmentation
    segmentation_img = np.zeros((image.shape[0], image.shape[1]))
    for id, m in masks_with_ids:
        segmentation_img[m["segmentation"]] = id

    rr.log("image/masks", rr.SegmentationImage(segmentation_img.astype(np.uint8)))

    mask_bbox = np.array([m["bbox"] for _, m in masks_with_ids])
    rr.log(
        "image/boxes",
        rr.Boxes2D(array=mask_bbox, array_format=rr.Box2DFormat.XYWH, class_ids=[id for id, _ in masks_with_ids]),
    )


def is_url(path: str) -> bool:
    """Check if a path is a url or a local file."""
    try:
        result = urlparse(path)
        return all([result.scheme, result.netloc])
    except ValueError:
        return False


def load_image(image_uri: str) -> cv2.typing.MatLike:
    """Conditionally download an image from URL or load it from disk."""
    logging.info(f"Loading: {image_uri}")
    if is_url(image_uri):
        response = requests.get(image_uri)
        response.raise_for_status()
        image_data = np.asarray(bytearray(response.content), dtype="uint8")
        image = cv2.imdecode(image_data, cv2.IMREAD_COLOR)
    else:
        image = cv2.imread(image_uri, cv2.IMREAD_COLOR)

    # Rerun can handle BGR as well, but SAM requires RGB.
    image = cv2.cvtColor(image, cv2.COLOR_BGR2RGB)
    return image


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Run the Facebook Research Segment Anything example.",
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )
    parser.add_argument(
        "--model",
        action="store",
        default="vit_b",
        choices=MODEL_URLS.keys(),
        help="Which model to use.(See: https://github.com/facebookresearch/segment-anything#model-checkpoints)",
    )
    parser.add_argument(
        "--device",
        action="store",
        default="cpu",
        help="Which torch device to use, e.g. cpu or cuda. "
        "(See: https://pytorch.org/docs/stable/tensor_attributes.html#torch.device)",
    )
    parser.add_argument(
        "--points-per-batch",
        action="store",
        default=32,
        type=int,
        help="Points per batch. More points will run faster, but too many will exhaust GPU memory.",
    )
    parser.add_argument("images", metavar="N", type=str, nargs="*", help="A list of images to process.")

    rr.script_add_args(parser)
    args = parser.parse_args()

    blueprint = rrb.Vertical(
        rrb.Spatial2DView(name="Image and segmentation mask", origin="/image"),
        rrb.Horizontal(
            rrb.TextLogView(name="Log", origin="/logs"),
            rrb.TextDocumentView(name="Description", origin="/description"),
            column_shares=[2, 1],
        ),
        row_shares=[3, 1],
    )

    rr.script_setup(args, "rerun_example_segment_anything_model", default_blueprint=blueprint)
    logging.getLogger().addHandler(rr.LoggingHandler("logs"))
    logging.getLogger().setLevel(logging.INFO)

    rr.log("description", rr.TextDocument(DESCRIPTION, media_type=rr.MediaType.MARKDOWN), static=True)

    sam = create_sam(args.model, args.device)

    mask_config = {"points_per_batch": args.points_per_batch}
    mask_generator = SamAutomaticMaskGenerator(sam, **mask_config)

    if len(args.images) == 0:
        logging.info("No image provided. Using default.")
        args.images = [
            "https://raw.githubusercontent.com/facebookresearch/segment-anything/main/notebooks/images/truck.jpg",
        ]

    for n, image_uri in enumerate(args.images):
        rr.set_time("image", sequence=n)
        image = load_image(image_uri)
        run_segmentation(mask_generator, image)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
