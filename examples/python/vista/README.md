<!--[metadata]
title = "Vista driving world model"
tags = ["2D", "Diffusion", "HuggingFace", "Video"]
source = "https://github.com/rerun-io/hf-example-vista"
thumbnail = "https://static.rerun.io/vista/1db07fa2bffee2351066e1768be5c7c72f9af0aa/480w.png"
thumbnail_dimensions = [480, 480]
-->


https://vimeo.com/969623509?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=2802:1790

## Background
Vista is a generative driving world model. Built on Stable Video Diffusion it can generate driving scenes conditioned on a single input image and optional, additional control inputs. In this example we visualize the latent diffusion steps and the generated, decoded image sequence.

## Run the code
This is an external example, check the [repository](https://github.com/rerun-io/hf-example-vista) for more information.

You can try the example on Rerun's HuggingFace space [here](https://huggingface.co/spaces/rerun/Vista).

If you have a GPU with ~20GB of memory you can run the example locally. To do so, clone the repo and run:
```
pixi run example
```
