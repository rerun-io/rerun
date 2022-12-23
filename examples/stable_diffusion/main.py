#!/usr/bin/env python3
"""Example running Depth Guided Stable Diffusion 2.0.

For more info see: https://github.com/Stability-AI/stablediffusion

"""
import argparse
import os
import platform
from pathlib import Path
from typing import Final

if platform.system() == "Darwin":
    os.environ["PYTORCH_ENABLE_MPS_FALLBACK"] = "1"


import requests
import torch
from huggingface_pipeline import StableDiffusionDepth2ImgPipeline
from PIL import Image

import rerun as rr

EXAMPLE_DIR: Final = Path(os.path.dirname(__file__))
DATASET_DIR: Final = EXAMPLE_DIR / "dataset"
IMAGE_PATH: Final = DATASET_DIR / "portrait-emil.jpg"

IMAGE_NAME_TO_URL: Final = {
    "old_man": "https://github.com/Stability-AI/stablediffusion/raw/main/assets/stable-samples/depth2img/old_man.png",
    "fantasy": "https://github.com/Stability-AI/stablediffusion/raw/main/assets/stable-samples/depth2img/depth2fantasy.jpeg",
}
IMAGE_NAMES: Final = list(IMAGE_NAME_TO_URL.keys())


def run_stable_diffusion(
    image_path: str, prompt: str, n_prompt: str, strength: float, guidance_scale: float, num_inference_steps: int
) -> None:

    pipe = StableDiffusionDepth2ImgPipeline.from_pretrained(
        "stabilityai/stable-diffusion-2-depth", local_files_only=True
    )

    if hasattr(torch.backends, "mps") and torch.backends.mps.is_available():
        pipe = pipe.to("mps")
    elif torch.cuda.is_available():
        pipe = pipe.to("cuda")
    else:
        pipe = pipe.to("cpu")

    pipe.enable_attention_slicing()

    image = Image.open(image_path)

    pipe(
        prompt=prompt,
        strength=strength,
        guidance_scale=11,
        negative_prompt=n_prompt,
        num_inference_steps=70,
        image=image,
    )


def get_downloaded_path(dataset_dir: Path, image_name: str) -> str:
    image_url = IMAGE_NAME_TO_URL[image_name]
    image_file_name = image_url.split("/")[-1]
    destination_path = dataset_dir / image_file_name
    if destination_path.exists():
        print(f"{destination_path} already exists. No need to download")
        return str(destination_path)

    print(f"Downloading video from {image_url} to {destination_path}")
    os.makedirs(dataset_dir.absolute(), exist_ok=True)
    with requests.get(image_url, stream=True) as req:
        req.raise_for_status()
        with open(destination_path, "wb") as f:
            for chunk in req.iter_content(chunk_size=8192):
                f.write(chunk)
    return str(destination_path)


def main() -> None:
    parser = argparse.ArgumentParser(description="Logs Objectron data using the Rerun SDK.")
    parser.add_argument("--headless", action="store_true", help="Don't show GUI")
    parser.add_argument("--connect", dest="connect", action="store_true", help="Connect to an external viewer")
    parser.add_argument("--addr", type=str, default=None, help="Connect to this ip:port")
    parser.add_argument("--save", type=str, default=None, help="Save data to a .rrd file at this path")
    parser.add_argument(
        "--image",
        type=str,
        default=IMAGE_NAMES[0],
        choices=IMAGE_NAMES,
        help="The example image to run on.",
    )
    parser.add_argument("--dataset_dir", type=Path, default=DATASET_DIR, help="Directory to save example images to.")
    parser.add_argument("--image_path", type=str, default="", help="Full path to image to run on. Overrides `--image`.")

    parser.add_argument(
        "--prompt",
        type=str,
        help="Positive prompt describing the image you want to generate.",
        default="Old man in a smoky disco wearing futuristic glasses.",
    )
    parser.add_argument(
        "--n_prompt",
        type=str,
        help="Negative prompt describing what you don't want in the image you generate.",
        default=None,
    )
    parser.add_argument(
        "--strength",
        type=float,
        default=0.65,
        help="""
Conceptually, indicates how much to transform the reference `image`. Must be between 0 and 1. `image`
will be used as a starting point, adding more noise to it the larger the `strength`. The number of
denoising steps depends on the amount of noise initially added. When `strength` is 1, added noise will
be maximum and the denoising process will run for the full number of iterations specified in
`num_inference_steps`. A value of 1, therefore, essentially ignores `image`.
""",
    )
    parser.add_argument(
        "--guidance_scale",
        type=float,
        default=8.5,
        help="""
Guidance scale as defined in [Classifier-Free Diffusion Guidance](https://arxiv.org/abs/2207.12598).
`guidance_scale` is defined as `w` of equation 2. of [Imagen
Paper](https://arxiv.org/pdf/2205.11487.pdf). Guidance scale is enabled by setting `guidance_scale >
1`. Higher guidance scale encourages to generate images that are closely linked to the text `prompt`,
usually at the expense of lower image quality.
""",
    )
    parser.add_argument(
        "--num_inference_steps",
        type=int,
        default=50,
        help="""
The number of denoising steps. More denoising steps usually lead to a higher quality image at the
expense of slower inference. This parameter will be modulated by `strength`.
""",
    )

    args = parser.parse_args()

    rr.init("Depth Guided Stable Diffusion ")

    image_path = args.image_path  # type: str
    if not image_path:
        image_path = get_downloaded_path(args.dataset_dir, args.image)

    if args.connect:
        # Send logging data to separate `rerun` process.
        # You can ommit the argument to connect to the default address,
        # which is `127.0.0.1:9876`.
        rr.connect(args.addr)

    run_stable_diffusion(
        image_path=image_path,
        prompt=args.prompt,
        n_prompt=args.n_prompt,
        strength=args.strength,
        guidance_scale=args.guidance_scale,
        num_inference_steps=args.num_inference_steps,
    )

    if args.save is not None:
        rr.save(args.save)
    elif args.headless:
        pass
    elif not args.connect:
        rr.show()


if __name__ == "__main__":
    main()
