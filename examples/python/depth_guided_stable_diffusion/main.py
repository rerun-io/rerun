#!/usr/bin/env python3
"""
Example running Depth Guided Stable Diffusion 2.0.

For more info see: https://github.com/Stability-AI/stablediffusion
"""
from __future__ import annotations

import argparse
import os
import platform
from pathlib import Path
from typing import Final

if platform.system() == "Darwin":
    os.environ["PYTORCH_ENABLE_MPS_FALLBACK"] = "1"

import requests
import rerun as rr  # pip install rerun-sdk
import rerun.blueprint as rrb
import torch
from huggingface_pipeline import StableDiffusionDepth2ImgPipeline
from PIL import Image

EXAMPLE_DIR: Final = Path(os.path.dirname(__file__))
DATASET_DIR: Final = EXAMPLE_DIR / "dataset"
CACHE_DIR: Final = EXAMPLE_DIR / "cache"

IMAGE_NAME_TO_URL: Final = {
    "sitting_wooden_figure": "https://storage.googleapis.com/rerun-example-datasets/stable_diffusion/sitting_wooden_figure.jpg",  # noqa: E501 line too long
    "old_man": "https://github.com/Stability-AI/stablediffusion/raw/main/assets/stable-samples/depth2img/old_man.png",  # noqa: E501 line too long
    "fantasy": "https://github.com/Stability-AI/stablediffusion/raw/main/assets/stable-samples/depth2img/depth2fantasy.jpeg",  # noqa: E501 line too long
}
IMAGE_NAMES: Final = list(IMAGE_NAME_TO_URL.keys())


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
    parser = argparse.ArgumentParser(description="Stable diffusion.")
    parser.add_argument(
        "--image",
        type=str,
        default=IMAGE_NAMES[0],
        choices=IMAGE_NAMES,
        help="The example image to run on.",
    )
    parser.add_argument("--dataset-dir", type=Path, default=DATASET_DIR, help="Directory to save example images to.")
    parser.add_argument("--image-path", type=str, default="", help="Full path to image to run on. Overrides `--image`.")

    parser.add_argument(
        "--prompt",
        type=str,
        help="Positive prompt describing the image you want to generate.",
        default="A tired robot sitting down on a dirt floor. Rusty metal. Unreal Engine. Wall-e",
    )
    parser.add_argument(
        "--n-prompt",
        type=str,
        help="Negative prompt describing what you don't want in the image you generate.",
        default="White uniform floor and background",
    )
    parser.add_argument(
        "--strength",
        type=float,
        default=0.7,
        help="""
Conceptually, indicates how much to transform the reference `image`. Must be between 0 and 1. `image`
will be used as a starting point, adding more noise to it the larger the `strength`. The number of
denoising steps depends on the amount of noise initially added. When `strength` is 1, added noise will
be maximum and the denoising process will run for the full number of iterations specified in
`num_inference_steps`. A value of 1, therefore, essentially ignores `image`.
""",
    )
    parser.add_argument(
        "--guidance-scale",
        type=float,
        default=11,
        help="""
Guidance scale as defined in [Classifier-Free Diffusion Guidance](https://arxiv.org/abs/2207.12598).
`guidance_scale` is defined as `w` of equation 2. of [Imagen
Paper](https://arxiv.org/pdf/2205.11487.pdf). Guidance scale is enabled by setting `guidance_scale >
1`. Higher guidance scale encourages to generate images that are closely linked to the text `prompt`,
usually at the expense of lower image quality.
""",
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

    rr.script_setup(
        args,
        "rerun_example_depth_guided_stable_diffusion",
        # This example is very complex, making it too hard for the Viewer to infer a good layout.
        # Therefore, we specify everything explicitly:
        # We set up three columns using a `Vertical` layout, one each for
        # * inputs
        # * depth & initializations
        # * diffusion outputs
        blueprint=rrb.Blueprint(
            rrb.Horizontal(
                rrb.Vertical(
                    rrb.Tabs(
                        rrb.Spatial2DView(name="Image original", origin="image/original"),
                        rrb.TensorView(name="Image preprocessed", origin="input_image/preprocessed"),
                    ),
                    rrb.Vertical(
                        rrb.TextLogView(name="Prompt", contents=["prompt/text", "prompt/text_negative"]),
                        rrb.Tabs(
                            rrb.TensorView(name="Text embeddings", origin="prompt/text_embeddings"),
                            rrb.TensorView(name="Unconditional embeddings", origin="prompt/uncond_embeddings"),
                        ),
                        rrb.BarChartView(name="Prompt ids", origin="prompt/text_input"),
                    ),
                ),
                rrb.Vertical(
                    rrb.Tabs(
                        rrb.Spatial2DView(name="Depth estimated", origin="depth/estimated"),
                        rrb.Spatial2DView(name="Depth interpolated", origin="depth/interpolated"),
                        rrb.Spatial2DView(name="Depth normalized", origin="depth/normalized"),
                        rrb.TensorView(name="Depth input pre-processed", origin="depth/input_preprocessed"),
                        active_tab="Depth interpolated",
                    ),
                    rrb.Tabs(
                        rrb.TensorView(name="Encoded input", origin="encoded_input_image"),
                        rrb.TensorView(name="Decoded init latents", origin="decoded_init_latents"),
                    ),
                ),
                rrb.Vertical(
                    rrb.Spatial2DView(name="Image diffused", origin="image/diffused"),
                    rrb.Horizontal(
                        rrb.TensorView(name="Latent Model Input", origin="diffusion/latent_model_input"),
                        rrb.TensorView(name="Diffusion latents", origin="diffusion/latents"),
                        # rrb.TensorView(name="Noise Prediction", origin="diffusion/noise_pred"),
                    ),
                ),
            ),
            rrb.SelectionPanel(expanded=False),
            rrb.TimePanel(expanded=False),
        ),
    )

    image_path = args.image_path  # type: str
    if not image_path:
        image_path = get_downloaded_path(args.dataset_dir, args.image)

    pipe = StableDiffusionDepth2ImgPipeline.from_pretrained(
        "stabilityai/stable-diffusion-2-depth", local_files_only=False, cache_dir=CACHE_DIR.absolute()
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
        prompt=args.prompt,
        strength=args.strength,
        guidance_scale=args.guidance_scale,
        negative_prompt=args.n_prompt,
        num_inference_steps=args.num_inference_steps,
        image=image,
    )

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
