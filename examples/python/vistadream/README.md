<!--[metadata]
title = "VistaDream: sampling multiview consistent images for single-view scene reconstruction"
tags = ["3D", "Reconstruction", "Pinhole camera", "Diffusion", "Single-view", "Gaussian splatting", "Novel views"]
source = "https://github.com/rerun-io/vistadream"
thumbnail = "https://static.rerun.io/vistadream/3d632e8e5e435b3d7d88860058ad8be071c5dc8a/480w.png"
thumbnail_dimensions = [480, 267]
-->

https://vimeo.com/1136303951?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=2696:1463

[VistaDream](https://vistadream-project-page.github.io/) is a novel framework for reconstructing 3D scenes from single-view images using Flux-based diffusion models. This implementation combines image outpainting, depth estimation, and 3D Gaussian splatting for high-quality 3D scene generation, with integrated visualization using [Rerun](https://rerun.io/).

## Background

VistaDream addresses the challenge of 3D scene reconstruction from a single image through a novel two-stage pipeline:

1. **Coarse 3D Scaffold Construction**: Creates a global scene structure by outpainting image boundaries and estimating depth maps.
2. **Multi-view Consistency Sampling**: Uses iterative diffusion-based RGB-D inpainting with multi-view consistency constraints to generate high-quality novel views.

The framework utilizes:

- **Flux diffusion models** for high-quality image outpainting and inpainting.
- **3D Gaussian Splatting** for efficient 3D scene representation.
- **Rerun** for real-time 3D visualization and debugging.

## Run the code

This is an external example. Check the [repository](https://github.com/rerun-io/vistadream) for more information.

**Requires**: Linux with an NVIDIA GPU (tested with CUDA 12.9)

Make sure you have the [Pixi package manager](https://pixi.sh/latest/#installation) installed and run:

```sh
git clone https://github.com/rerun-io/vistadream.git
cd vistadream
pixi run example
```
