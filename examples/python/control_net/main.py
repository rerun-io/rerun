import argparse
import os
from pathlib import Path
from typing import Final

import cv2
import numpy as np
import requests
import rerun as rr
import tomesd
import torch
from diffusers import ControlNetModel, UniPCMultistepScheduler
from huggingface_pipeline import StableDiffusionControlNetPipeline
from PIL import Image

EXAMPLE_DIR: Final = Path(os.path.dirname(__file__))
DATASET_DIR: Final = EXAMPLE_DIR / "dataset"
CACHE_DIR: Final = EXAMPLE_DIR / "cache"

IMAGE_NAME_TO_URL: Final = {
    "vermeer": "https://hf.co/datasets/huggingface/documentation-images/resolve/main/diffusers/input_image_vermeer.png",  # noqa: E501 line too long
}
IMAGE_NAMES: Final = list(IMAGE_NAME_TO_URL.keys())
CONTROL_TYPE = ["canny"]


def get_downloaded_path(dataset_dir: Path, image_name: str) -> str:
    image_url = IMAGE_NAME_TO_URL[image_name]
    image_file_name = image_url.split("/")[-1]
    destination_path = dataset_dir / image_file_name
    if destination_path.exists():
        print(f"{destination_path} already exists. No need to download")
        return str(destination_path)

    print(f"Downloading image from {image_url} to {destination_path}")
    os.makedirs(dataset_dir.absolute(), exist_ok=True)
    with requests.get(image_url, stream=True) as req:
        req.raise_for_status()
        with open(destination_path, "wb") as f:
            for chunk in req.iter_content(chunk_size=8192):
                f.write(chunk)
    return str(destination_path)

def get_canny_filter(image):
    if not isinstance(image, np.ndarray):
        image = np.array(image)

    image = cv2.Canny(image, 100, 200)
    image = image[:, :, None]
    image = np.concatenate([image, image, image], axis=2)
    canny_image = Image.fromarray(image)
    return canny_image


def main() -> None:
    parser = argparse.ArgumentParser(description="Logs Objectron data using the Rerun SDK.")
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
        default="Idris Elba, best quality, extremely detailed",
    )

    parser.add_argument(
        "--n_prompt",
        type=str,
        help="Negative prompt describing what you don't want in the image you generate.",
        default="longbody,lowres, bad anatomy, bad hands, missing fingers, extra digit, fewer digits, cropped, worst quality, low quality",
    )

    # parser.add_argument(
    #     "--control-type",
    #     type=str,
    #     help="Positive prompt describing the image you want to generate.",
    #     default="Emilia Clarke, best quality, extremely detailed",
    # )
    parser.add_argument(
        "--num_inference_steps",
        type=int,
        default=10,
        help="""
The number of denoising steps. More denoising steps usually lead to a higher quality image at the
expense of slower inference. This parameter will be modulated by `strength`.
""",
    )

    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "Canny Edge ControlNet")

    image_path = args.image_path  # type: str
    if not image_path:
        image_path = get_downloaded_path(args.dataset_dir, args.image)

    # Models
    controlnet = ControlNetModel.from_pretrained("lllyasviel/sd-controlnet-canny")#, torch_dtype=torch.float16)
    pipe = StableDiffusionControlNetPipeline.from_pretrained(
        "runwayml/stable-diffusion-v1-5", controlnet=controlnet, safety_checker=None)#, torch_dtype=torch.float16

    # tomesd.apply_patch(pipe, ratio=0.5) # Can also use pipe.unet in place of pipe here

    pipe.scheduler = UniPCMultistepScheduler.from_config(pipe.scheduler.config)

    if torch.cuda.is_available():
        pipe = pipe.to("cuda")
    else:
        pipe = pipe.to("cpu")

    pipe.enable_attention_slicing()

    image = Image.open(image_path)

    canny_image = get_canny_filter(image)
    # Generator seed,
    torch.manual_seed(0)
    output = pipe(
        args.prompt,
        canny_image,
        negative_prompt=args.n_prompt,
        num_inference_steps=args.num_inference_steps,
    )

    rr.log_text_entry("prompt", args.prompt)
    rr.log_image('canny_image', canny_image)
    rr.log_image('image', image)
    rr.log_image('output', output.images[0])
    rr.script_teardown(args)


if __name__ == "__main__":
    main()
