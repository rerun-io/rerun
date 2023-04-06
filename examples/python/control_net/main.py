import argparse
import os
from pathlib import Path
from typing import Final

import requests
import rerun as rr
import tomesd
import torch
from controlnet_aux import CannyDetector, MidasDetector, HEDdetector, OpenposeDetector, MLSDdetector
from diffusers import ControlNetModel, UniPCMultistepScheduler
from huggingface_pipeline import StableDiffusionControlNetPipeline
from PIL import Image

torch.manual_seed(0)

EXAMPLE_DIR: Final = Path(os.path.dirname(__file__))
DATASET_DIR: Final = EXAMPLE_DIR / "dataset"
CACHE_DIR: Final = EXAMPLE_DIR / "cache"

IMAGE_NAME_TO_URL: Final = {
    "vermeer": "https://hf.co/datasets/huggingface/documentation-images/resolve/main/diffusers/input_image_vermeer.png",  # noqa: E501 line too long
}
IMAGE_NAMES: Final = list(IMAGE_NAME_TO_URL.keys())
CONTROLNET_MODEL_IDS = {
    "canny": "lllyasviel/sd-controlnet-canny",
    "hough": "lllyasviel/sd-controlnet-mlsd",
    "hed": "lllyasviel/sd-controlnet-hed",
    "pose": "lllyasviel/sd-controlnet-openpose",
    "depth": "lllyasviel/sd-controlnet-depth",
    "normal": "lllyasviel/sd-controlnet-normal",
}
DEVICE = "cuda" if torch.cuda.is_available() else "cpu"


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

    parser.add_argument(
        "--control-type",
        type=str,
        help="Positive prompt describing the image you want to generate.",
        default="canny",
        choices=CONTROLNET_MODEL_IDS.keys(),
    )
    parser.add_argument(
        "--num-inference-steps",
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
    controlnet = ControlNetModel.from_pretrained(CONTROLNET_MODEL_IDS[args.control_type])
    pipe = StableDiffusionControlNetPipeline.from_pretrained(
        "runwayml/stable-diffusion-v1-5", controlnet=controlnet, safety_checker=None
    )  # , torch_dtype=torch.float16

    pipe.scheduler = UniPCMultistepScheduler.from_config(pipe.scheduler.config)

    pipe = pipe.to(DEVICE)

    pipe.enable_attention_slicing()
    tomesd.apply_patch(pipe, ratio=0.5)

    image = Image.open(image_path)

    if args.control_type == "depth" or args.control_type == "normal":
        midas = MidasDetector.from_pretrained("lllyasviel/ControlNet")
        depth, normal = midas(image)
        if args.control_type == "depth":
            controlnet_input = depth
        else:
            controlnet_input = normal
    elif args.control_type == "canny":
        detector = CannyDetector()
        controlnet_input = detector(image, 100, 200)
    elif args.control_type == "hed":
        detector = HEDdetector.from_pretrained("lllyasviel/ControlNet")
        controlnet_input = detector(image)
    elif args.control_type == "hough":
        mlsd = MLSDdetector.from_pretrained("lllyasviel/ControlNet")
        controlnet_input = mlsd(image)
    elif args.control_type == "pose":
        pose = OpenposeDetector.from_pretrained("lllyasviel/ControlNet")
        controlnet_input = pose(image)

    else:
        raise NotImplementedError

    rr.log_image("original_image", image)

    output = pipe(
        args.prompt,
        controlnet_input,
        negative_prompt=args.n_prompt,
        num_inference_steps=args.num_inference_steps,
    )

    rr.log_image("image/diffused_image", output.images[0])
    rr.script_teardown(args)


if __name__ == "__main__":
    main()
