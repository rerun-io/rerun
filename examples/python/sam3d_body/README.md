<!--[metadata]
title = "SAM 3D body: robust Full-Body human mesh recovery"
tags = ["3D", "Human mesh", "Body tracking", "Single-view"]
source = "https://github.com/rerun-io/sam3d-body-rerun"
thumbnail = "https://static.rerun.io/sam3d-body/b8477f902c4fcdfd1168286193d2dc20e4ad9d20/480w.png"
thumbnail_dimensions = [480, 300]
-->

https://vimeo.com/1155605317?loop=1&autopause=0&background=1&muted=1&ratio=2802:1790

[SAM 3D Body](https://github.com/facebookresearch/sam-3d-body) is a promptable model for single-image full-body 3D human mesh recovery (HMR) from Meta. This example showcases an unofficial playground with promptable SAM3 masks and live [Rerun](https://rerun.io/) visualization, using Gradio for the UI and Pixi for one-command setup.

## Background

SAM 3D Body (3DB) demonstrates state-of-the-art performance for 3D human mesh recovery with strong generalization in diverse in-the-wild conditions.

Key features of the model:
- **Single-image HMR**: Reconstructs full 3D human mesh from a single image
- **Momentum Human Rig (MHR)**: Uses a parametric mesh representation that decouples skeletal structure and surface shape for improved accuracy
- **Promptable inference**: Supports auxiliary prompts including 2D keypoints and masks for user-guided reconstruction
- **Full-body estimation**: Estimates pose of body, feet, and hands

This Rerun integration provides:
- **Gradio App**: Interactive UI with embedded streaming Rerun viewer
- **CLI tools**: Batch processing capabilities
- **Video segmentation**: Single and multiview video processing with SAM3
- **Multiview optimization**: Fuses per-view body predictions into globally-consistent 3D mesh

## Run the code

This is an external example. Check the [repository](https://github.com/rerun-io/sam3d-body-rerun) for more information.

**Requirements**: Linux with an NVIDIA GPU

The SAM3 and SAM3D Body checkpoints are gated on Hugging Face. Request access for both [facebook/sam-3d-body-dinov3](https://huggingface.co/facebook/sam-3d-body-dinov3) and [facebook/sam3](https://huggingface.co/facebook/sam3), then authenticate by setting `HF_TOKEN=<your token>` or running `huggingface-cli login`.

Make sure you have the [Pixi package manager](https://pixi.sh/latest/#installation) installed and run:

```sh
git clone https://github.com/rerun-io/sam3d-body-rerun.git
cd sam3d-body-rerun
pixi run app
```
