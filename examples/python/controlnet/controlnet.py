#!/usr/bin/env python3
"""
Example running ControlNet conditioned on Canny edges.

Based on <https://huggingface.co/docs/diffusers/using-diffusers/controlnet>.
"""

from __future__ import annotations

import argparse
import os
from typing import Any

import cv2
import numpy as np
import PIL.Image
import requests
import rerun as rr
import rerun.blueprint as rrb
import torch
from diffusers import (
    AutoencoderKL,
    ControlNetModel,
    StableDiffusionXLControlNetPipeline,
)

RERUN_LOGO_URL = "https://storage.googleapis.com/rerun-example-datasets/controlnet/rerun-icon-1000.png"


def controlnet_callback(
    pipe: StableDiffusionXLControlNetPipeline,
    step_index: int,
    timestep: float,
    callback_kwargs: dict[str, Any],
) -> dict[str, Any]:
    rr.set_time("iteration", sequence=step_index)
    rr.set_time("timestep", duration=timestep)
    latents = callback_kwargs["latents"]

    image = pipe.vae.decode(latents / pipe.vae.config.scaling_factor, return_dict=False)[0]  # type: ignore[attr-defined]
    image = pipe.image_processor.postprocess(image, output_type="np").squeeze()  # type: ignore[attr-defined]
    rr.log("output", rr.Image(image))
    rr.log("latent", rr.Tensor(latents.squeeze(), dim_names=["channel", "height", "width"]))

    return callback_kwargs


def run_canny_controlnet(image_path: str, prompt: str, negative_prompt: str) -> None:
    if not torch.cuda.is_available():
        print("This example requires a torch with CUDA, but no CUDA device found. Aborting.")
        return

    if image_path.startswith(("http://", "https://")):
        pil_image = PIL.Image.open(requests.get(image_path, stream=True).content)
    elif os.path.isfile(image_path):
        pil_image = PIL.Image.open(image_path)
    else:
        raise ValueError(f"Invalid image_path: {image_path}")

    image = np.array(pil_image)

    if image.shape[2] == 4:  # RGBA image
        rgb_image = image[..., :3]  # RGBA to RGB
        rgb_image[image[..., 3] < 200] = 0.0  # reduces artifacts for transparent parts
    else:
        rgb_image = image

    low_threshold = 100.0
    high_threshold = 200.0
    canny_data = cv2.Canny(rgb_image, low_threshold, high_threshold)
    canny_data = canny_data[:, :, None]
    # cv2.dilate(kjgk
    canny_data = np.concatenate([canny_data, canny_data, canny_data], axis=2)
    canny_image = PIL.Image.fromarray(canny_data)

    rr.log("input/raw", rr.Image(image), static=True)
    rr.log("input/canny", rr.Image(canny_image), static=True)

    controlnet = ControlNetModel.from_pretrained(
        "diffusers/controlnet-canny-sdxl-1.0",
        torch_dtype=torch.float16,
        use_safetensors=True,
    )
    vae = AutoencoderKL.from_pretrained(
        "madebyollin/sdxl-vae-fp16-fix",
        torch_dtype=torch.float16,
        use_safetensors=True,
    )
    pipeline = StableDiffusionXLControlNetPipeline.from_pretrained(
        "stabilityai/stable-diffusion-xl-base-1.0",
        controlnet=controlnet,
        vae=vae,
        torch_dtype=torch.float16,
        use_safetensors=True,
    )

    pipeline.enable_model_cpu_offload()

    rr.log("positive_prompt", rr.TextDocument(prompt), static=True)
    rr.log("negative_prompt", rr.TextDocument(negative_prompt), static=True)

    images = pipeline(
        prompt,
        negative_prompt=negative_prompt,
        image=canny_image,  # add batch dimension
        controlnet_conditioning_scale=0.5,
        callback_on_step_end=controlnet_callback,
    ).images[0]

    rr.log("output", rr.Image(images))


def main() -> None:
    parser = argparse.ArgumentParser(description="Use Canny-conditioned ControlNet to generate image.")
    parser.add_argument(
        "--img-path",
        type=str,
        help="Path to image used as input for Canny edge detector.",
        default=RERUN_LOGO_URL,
    )
    parser.add_argument(
        "--prompt",
        type=str,
        help="Prompt used as input for ControlNet.",
        default="aerial view, a futuristic research complex in a bright foggy jungle, hard lighting",
    )
    parser.add_argument(
        "--negative-prompt",
        type=str,
        help="Negative prompt used as input for ControlNet.",
        default="low quality, bad quality, sketches",
    )
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(
        args,
        "rerun_example_controlnet",
        default_blueprint=rrb.Horizontal(
            rrb.Grid(
                rrb.Spatial2DView(origin="input/raw"),
                rrb.Spatial2DView(origin="input/canny"),
                rrb.Vertical(
                    rrb.TextDocumentView(origin="positive_prompt"),
                    rrb.TextDocumentView(origin="negative_prompt"),
                ),
                rrb.TensorView(origin="latent"),
            ),
            rrb.Spatial2DView(origin="output"),
        ),
    )
    run_canny_controlnet(args.img_path, args.prompt, args.negative_prompt)
    rr.script_teardown(args)


if __name__ == "__main__":
    main()
