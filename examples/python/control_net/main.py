import argparse

import cv2
import numpy as np
import rerun as rr
import torch
from diffusers import ControlNetModel, UniPCMultistepScheduler
from huggingface_pipeline import StableDiffusionControlNetPipeline
from PIL import Image

# Generator seed,
generator = torch.manual_seed(0)

def get_canny_filter(image):
    if not isinstance(image, np.ndarray):
        image = np.array(image)

    image = cv2.Canny(image, 75, 150)
    image = image[:, :, None]
    image = np.concatenate([image, image, image], axis=2)
    canny_image = Image.fromarray(image)
    return canny_image


def main() -> None:
    parser = argparse.ArgumentParser(description="Logs Objectron data using the Rerun SDK.")
    # parser.add_argument(
    #     "--image",
    #     type=str,
    #     default=IMAGE_NAMES[0],
    #     choices=IMAGE_NAMES,
    #     help="The example image to run on.",
    # )

    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "Depth Guided Stable Diffusion")

    image_path = "/Users/pablovela/0Dev/personal/rerun/examples/python/control_net/input_image_vermeer.png"  # type: str
    # Constants

    # Models
    controlnet = ControlNetModel.from_pretrained("lllyasviel/sd-controlnet-canny")#, torch_dtype=torch.float16)
    pipe = StableDiffusionControlNetPipeline.from_pretrained(
        "runwayml/stable-diffusion-v1-5", controlnet=controlnet, safety_checker=None)#, torch_dtype=torch.float16

    pipe.scheduler = UniPCMultistepScheduler.from_config(pipe.scheduler.config)

    if torch.cuda.is_available():
        pipe = pipe.to("cuda")
    else:
        pipe = pipe.to("cpu")

    pipe.enable_attention_slicing()

    image = Image.open(image_path)
    prompt = "Blake Lively, best quality, extremely detailed"

    canny_image = get_canny_filter(image)

    output = pipe(
        prompt,
        canny_image,
        generator=generator,
        num_images_per_prompt=1,
        num_inference_steps=5,
    )

    rr.log_text_entry("prompt", prompt)
    rr.log_image('canny_image', canny_image)
    rr.log_image('image', image)
    rr.log_image('output', output.images[0])
    rr.script_teardown(args)


if __name__ == "__main__":
    main()
