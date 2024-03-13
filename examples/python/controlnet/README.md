<!--[metadata]
title = "ControlNet"
tags = ["controlnet", "canny", "huggingface", "stable-diffusion", "tensor", "text"]
description = "Use Hugging Face's ControlNet to condition Stable Diffusion on edges detected by the Canny edge detector."
thumbnail = "https://static.rerun.io/controlnet/8aace9c59a423c2eeabe4b7f9abb5187559c52e8/480w.png"
thumbnail_dimensions = [480, 303]
-->

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/controlnet/8aace9c59a423c2eeabe4b7f9abb5187559c52e8/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/controlnet/8aace9c59a423c2eeabe4b7f9abb5187559c52e8/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/controlnet/8aace9c59a423c2eeabe4b7f9abb5187559c52e8/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/controlnet/8aace9c59a423c2eeabe4b7f9abb5187559c52e8/1200w.png">
  <img src="https://static.rerun.io/controlnet/8aace9c59a423c2eeabe4b7f9abb5187559c52e8/full.png" alt="">
</picture>

Use [Hugging Face's ControlNet](https://huggingface.co/docs/diffusers/using-diffusers/controlnet#controlnet) to condition Stable Diffusion on edges detected by the Canny edge detector.



## Used Rerun Types
[`Image`](https://www.rerun.io/docs/reference/types/archetypes/image), [`Tensor`](https://www.rerun.io/docs/reference/types/archetypes/tensor), [`TextDocument`](https://www.rerun.io/docs/reference/types/archetypes/text_document)


## Background
[Hugging Face's ControlNet](https://huggingface.co/docs/diffusers/using-diffusers/controlnet#controlnet). allows to condition Stable Diffusion on various modalities. In this example we condition on edges detected by the Canny edge detector to keep our shape intact while generating an image with Stable Diffusion.

https://vimeo.com/870289439?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=1440:1080

# Logging and Visualizing with Rerun
The visualizations in this example were created with the following Rerun code.

## Canny image
Uses cv2 to calculate a [canny image](https://docs.opencv.org/4.x/da/d22/tutorial_py_canny.html) that shows the edges in the image in black and white.
```python
canny_image = cv2.Canny(rgb_image, low_threshold, high_threshold)
canny_image = canny_image[:, :, None]
canny_image = np.concatenate([canny_image, canny_image, canny_image], axis=2)
canny_image = PIL.Image.fromarray(canny_image)
```

Log the input image and the canny image to Rerun.
```
rr.log("input/raw", rr.Image(image), timeless=True)
rr.log("input/canny", rr.Image(canny_image), timeless=True)
```


## Controlnet generation
Load the [ControlNet ](https://huggingface.co/diffusers/controlnet-canny-sdxl-1.0) and [Stable Diffusion XL](https://huggingface.co/stabilityai/stable-diffusion-xl-base-1.0) models from HuggingFace 
```python
controlnet = ControlNetModel.from_pretrained("diffusers/controlnet-canny-sdxl-1.0", torch_dtype=torch.float16, use_safetensors=True)
pipeline = StableDiffusionXLControlNetPipeline.from_pretrained("stabilityai/stable-diffusion-xl-base-1.0", controlnet=controlnet, vae=vae, torch_dtype=torch.float16, use_safetensors=True)
pipeline.enable_model_cpu_offload()
```

Generate a new image from prompt, negative prompt and canny image and log to Rerun.
```python
images = pipeline(prompt, negative_prompt=negative_prompt, image=canny_image, controlnet_conditioning_scale=0.5, callback=lambda i, t, latents: controlnet_callback(i, t, latents, pipeline)).images[0]
rr.log("output", rr.Image(images))
```

We use a custom callback function for ControlNet that logs the output and the latent values at each timestep, which makes it possible for us to view all timesteps of the generation in Rerun.
```python
def controlnet_callback(
    iteration: int, timestep: float, latents: torch.Tensor, pipeline: StableDiffusionXLControlNetPipeline
) -> None:
    rr.set_time_sequence("iteration", iteration)
    rr.set_time_seconds("timestep", timestep)

    image = pipeline.vae.decode(latents / pipeline.vae.config.scaling_factor, return_dict=False)[0]
    image = pipeline.image_processor.postprocess(image, output_type="np").squeeze()
    rr.log("output", rr.Image(image))
    rr.log("latent", rr.Tensor(latents.squeeze(), dim_names=["channel", "height", "width"]))
```

To run this example use
```bash
pip install -r examples/python/controlnet/requirements.txt
python examples/python/controlnet/main.py
```

You can specify your own image and prompts using
```bash
main.py [--img-path IMG_PATH] [--prompt PROMPT] [--negative-prompt NEGATIVE_PROMPT]
```
