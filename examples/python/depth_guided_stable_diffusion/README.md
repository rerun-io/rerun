<!--[metadata]
title = "Depth Guided Stable Diffusion"
tags = ["2D", "depth", "huggingface", "stable-diffusion", "tensor", "text"]
thumbnail = "https://static.rerun.io/depth_guided_stable_diffusion/a85516aba09f72649517891d767e15383ce7f4ea/480w.png"
thumbnail_dimensions = [480, 253]
channel = "nightly"
-->


<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/depth_guided_stable_diffusion/a85516aba09f72649517891d767e15383ce7f4ea/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/depth_guided_stable_diffusion/a85516aba09f72649517891d767e15383ce7f4ea/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/depth_guided_stable_diffusion/a85516aba09f72649517891d767e15383ce7f4ea/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/depth_guided_stable_diffusion/a85516aba09f72649517891d767e15383ce7f4ea/1200w.png">
  <img src="https://static.rerun.io/depth_guided_stable_diffusion/a85516aba09f72649517891d767e15383ce7f4ea/full.png" alt="Depth-guided stable diffusion screenshot">
</picture>

Visualize the outputs of the [Depth Guided Stable Diffusion 2.0](https://github.com/Stability-AI/stablediffusion) for different conditioning inputs.


## Used Rerun Types
[`Image`](https://www.rerun.io/docs/reference/types/archetypes/image), [`DepthImage`](https://www.rerun.io/docs/reference/types/archetypes/depth_image), [`Tensor`](https://www.rerun.io/docs/reference/types/archetypes/tensor), [`TextLog`](https://www.rerun.io/docs/reference/types/archetypes/text_log)

## Background

ControlNet allows to condition Stable Diffusion on various modalities. 
There are many types of conditioning inputs (canny edge, user sketching, human pose, depth, and more) to control a diffusion model.
This is hugely useful because it affords you greater control over image generation, making it easier to generate specific images without experimenting with different text prompts or denoising values as much.

# Run the Code

To run this example, make sure you have the Rerun repository checked out and the latest SDK installed:
```bash
# Setup
pip install --upgrade rerun-sdk  # install the latest Rerun SDK
git clone git@github.com:rerun-io/rerun.git  # Clone the repository
cd rerun
git checkout latest  # Check out the commit matching the latest SDK release
```

Install the necessary libraries specified in the requirements file:
```bash
pip install -r examples/python/depth_guided_stable_diffusion/requirements.txt
```
To experiment with the provided example, simply execute the main Python script:
```bash
python examples/python/depth_guided_stable_diffusion/main.py # run the example
```

If you wish to customize it, explore additional features, or save it use the CLI with the `--help` option for guidance:

```bash
python examples/python/depth_guided_stable_diffusion/main.py --help

usage: main.py [-h] [--image {sitting_wooden_figure,old_man,fantasy}] [--dataset-dir DATASET_DIR] [--image-path IMAGE_PATH] [--prompt PROMPT] [--n-prompt N_PROMPT]
               [--strength STRENGTH] [--guidance-scale GUIDANCE_SCALE] [--num-inference-steps NUM_INFERENCE_STEPS] [--headless] [--connect] [--serve] [--addr ADDR]
               [--save SAVE] [-o]

Stable diffusion.

optional arguments:
  -h, --help            show this help message and exit
  --image {sitting_wooden_figure,old_man,fantasy}
                        The example image to run on.
  --dataset-dir DATASET_DIR
                        Directory to save example images to.
  --image-path IMAGE_PATH
                        Full path to image to run on. Overrides `--image`.
  --prompt PROMPT       Positive prompt describing the image you want to generate.
  --n-prompt N_PROMPT   Negative prompt describing what you don t want in the image you generate.
  --strength STRENGTH   Conceptually, indicates how much to transform the reference `image`. Must be between 0 and 1. `image` will be used as a starting point,
                        adding more noise to it the larger the `strength`. The number of denoising steps depends on the amount of noise initially added. When
                        `strength` is 1, added noise will be maximum and the denoising process will run for the full number of iterations specified in
                        `num_inference_steps`. A value of 1, therefore, essentially ignores `image`.
  --guidance-scale GUIDANCE_SCALE
                        Guidance scale as defined in [Classifier-Free Diffusion Guidance](https://arxiv.org/abs/2207.12598). `guidance_scale` is defined as `w` of
                        equation 2. of [Imagen Paper](https://arxiv.org/pdf/2205.11487.pdf). Guidance scale is enabled by setting `guidance_scale > 1`. Higher
                        guidance scale encourages to generate images that are closely linked to the text `prompt`, usually at the expense of lower image quality.
  --num-inference-steps NUM_INFERENCE_STEPS
                        The number of denoising steps. More denoising steps usually lead to a higher quality image at the expense of slower inference. This parameter
                        will be modulated by `strength`.
  --headless            Don t show GUI
  --connect             Connect to an external viewer
  --serve               Serve a web viewer (WARNING: experimental feature)
  --addr ADDR           Connect to this ip:port
  --save SAVE           Save data to a .rrd file at this path
  -o, --stdout          Log data to standard output, to be piped into a Rerun Viewer

```
