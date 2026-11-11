<!--[metadata]
title = "EgoHand: 3D hands from egocentric video"
description = "Find both hands in a head-camera video and turn each one into metric 3D joints and a SOMA hand mesh with NVIDIA's EgoHand, step by step."
tags = ["2D", "3D", "Egocentric", "Human mesh", "Keypoint detection", "Hugging Face"]
source = "https://github.com/rerun-io/examples-monorepo/tree/main/packages/posekit"
thumbnail = "https://static.rerun.io/egohand_thumbnail_768x576/a8e0f1124229fe08965ddd318366b47abae2fb56/full.png"
thumbnail_dimensions = [768, 576]
preview_rrd_url = "https://huggingface.co/datasets/pablovela5620/egohand-rerun-example/resolve/main/egohand-gen-humanego-segment.rrd"
-->

[![Open in Hugging Face Spaces](https://huggingface.co/datasets/huggingface/badges/resolve/main/open-in-hf-spaces-md.svg)](https://pablovela5620-egohand-rerun.hf.space)

```agent-prompt
```

Try it in your browser with no install (the Space runs the fast path on ZeroGPU), or copy a prompt for your coding agent to install and run it on your own Linux machine with an NVIDIA GPU.

<video src="https://static.rerun.io/b878cfc3171606bada00d086160db9511731e6d2_seg-run.mp4" poster="https://static.rerun.io/d64429345c4839f7fdcd009a0a43c709d2198064_seg-run.webp" autoplay muted loop playsinline></video>

EgoHand finds both hands in each frame of a head-worn camera and turns each hand into a metric 3D mesh.
This example runs [NVIDIA's EgoHand](https://huggingface.co/nvidia/EgoHand-v1) on a head-camera video and logs every step to Rerun: the video with hand boxes and 2D joints, and a 3D view with the camera, the 3D joints and a SOMA hand mesh per hand.

## The result

The video above is 30 s of a [Gen-HumanEgo](https://huggingface.co/datasets/genrobot2025/Gen-HumanEgo) head camera: a person hangs shirts on hangers.
Left: the fisheye frame with boxes and 2D joints.
Right: 3D joints and SOMA hand meshes in the camera frame.
Both hands are found on all 900 frames.
The CLI takes a pinhole camera, so this run gives it the lens's focal length and center without the double-sphere distortion: the 3D placement is an approximation. Steps 1 and 3 below show the real double-sphere lens.

The same 900 frames, one at a time, with detection on every frame and full meshes (decode not timed), on an NVIDIA DGX Spark (GB10):

| Config (this 900-frame clip) | ms per frame | fps | Speed-up |
|---|---|---|---|
| Reference (`egohand`) | 187.56 | 5.3 | 1.00x |
| Fast path (`egohand-fast`: TensorRT, NVFP4/FP8 backbone, CUDA graphs) | 18.25 | 54.8 | 10.28x |

Where the time goes, per stage, reference (left) against the fast path (right):

<picture>
  <img src="https://static.rerun.io/egohand_seg_stages/b963bf82fd6b4a614d084259d3ba96a84271968d/full.webp" alt="Runtime per stage: reference against fast path">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/egohand_seg_stages/b963bf82fd6b4a614d084259d3ba96a84271968d/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/egohand_seg_stages/b963bf82fd6b4a614d084259d3ba96a84271968d/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/egohand_seg_stages/b963bf82fd6b4a614d084259d3ba96a84271968d/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/egohand_seg_stages/b963bf82fd6b4a614d084259d3ba96a84271968d/1200w.webp">
</picture>

## How one frame goes through it

Frame 34 of the run above: both hands hold a white shirt on a hanger.

<picture>
  <img src="https://static.rerun.io/egohand_egohand_flow/860a9b8e0f4a213c74b4d13be34021174daee28a/full.webp" alt="The pipeline on frame 34: camera frame, RT-DETR hand detector, hand crops, DINOv3, heads, MHR / SOMA mesh">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/egohand_egohand_flow/860a9b8e0f4a213c74b4d13be34021174daee28a/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/egohand_egohand_flow/860a9b8e0f4a213c74b4d13be34021174daee28a/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/egohand_egohand_flow/860a9b8e0f4a213c74b4d13be34021174daee28a/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/egohand_egohand_flow/860a9b8e0f4a213c74b4d13be34021174daee28a/1200w.webp">
</picture>

### 1. The camera frame and its lens

The double-sphere lens sees 163° across, so straight lines bend. EgoHand never undistorts the whole frame, only one small view per hand.

<picture>
  <img src="https://static.rerun.io/egohand_walk_1_frame/c836401623cb8297a5c752e8d14f920855b3efb7/full.webp" alt="The camera frame and its lens">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/egohand_walk_1_frame/c836401623cb8297a5c752e8d14f920855b3efb7/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/egohand_walk_1_frame/c836401623cb8297a5c752e8d14f920855b3efb7/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/egohand_walk_1_frame/c836401623cb8297a5c752e8d14f920855b3efb7/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/egohand_walk_1_frame/c836401623cb8297a5c752e8d14f920855b3efb7/1200w.webp">
</picture>

### 2. Find the hands

An [RT-DETR](https://arxiv.org/abs/2304.08069) hand detector sees a 640 × 640 copy of the frame and returns a box and a side for each hand.

<picture>
  <img src="https://static.rerun.io/egohand_walk_2_detector/9eb0eff018ce425e3e8f6772e96fdd626b6ba72a/full.webp" alt="Find the hands">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/egohand_walk_2_detector/9eb0eff018ce425e3e8f6772e96fdd626b6ba72a/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/egohand_walk_2_detector/9eb0eff018ce425e3e8f6772e96fdd626b6ba72a/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/egohand_walk_2_detector/9eb0eff018ce425e3e8f6772e96fdd626b6ba72a/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/egohand_walk_2_detector/9eb0eff018ce425e3e8f6772e96fdd626b6ba72a/1200w.webp">
</picture>

### 3. Crop each hand through the lens

A virtual pinhole camera turns toward each hand and samples the frame through the lens, so the crops come out straight. The right crop is mirrored, so the model always sees a left hand.

<picture>
  <img src="https://static.rerun.io/egohand_walk_3_crops/6669a2d2e6ee2d878fb9e41f3c64c6ea695276ad/full.webp" alt="Crop each hand through the lens">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/egohand_walk_3_crops/6669a2d2e6ee2d878fb9e41f3c64c6ea695276ad/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/egohand_walk_3_crops/6669a2d2e6ee2d878fb9e41f3c64c6ea695276ad/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/egohand_walk_3_crops/6669a2d2e6ee2d878fb9e41f3c64c6ea695276ad/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/egohand_walk_3_crops/6669a2d2e6ee2d878fb9e41f3c64c6ea695276ad/1200w.webp">
</picture>

### 4. Backbone features

[DINOv3](https://arxiv.org/abs/2508.10104) ViT-H+/16 turns each crop into 256 tokens through 32 blocks.

<picture>
  <img src="https://static.rerun.io/egohand_walk_4_backbone/cb1c4dfa2b40136905349dc7210e3a7e1bd774f1/full.webp" alt="DINOv3 features">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/egohand_walk_4_backbone/cb1c4dfa2b40136905349dc7210e3a7e1bd774f1/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/egohand_walk_4_backbone/cb1c4dfa2b40136905349dc7210e3a7e1bd774f1/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/egohand_walk_4_backbone/cb1c4dfa2b40136905349dc7210e3a7e1bd774f1/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/egohand_walk_4_backbone/cb1c4dfa2b40136905349dc7210e3a7e1bd774f1/1200w.webp">
</picture>

### 5. Heads: pose, shape, depth and 2D joints

The heads give 25 joint rotations, the hand's shape, a depth and 25 direct 2D joints per hand. The 2D joints place the hand sideways.

<picture>
  <img src="https://static.rerun.io/egohand_walk_5_heads/5183514109ffa6face065b5b0ba2a5e46ee4e96c/full.webp" alt="Heads: pose, shape, depth and 2D joints">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/egohand_walk_5_heads/5183514109ffa6face065b5b0ba2a5e46ee4e96c/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/egohand_walk_5_heads/5183514109ffa6face065b5b0ba2a5e46ee4e96c/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/egohand_walk_5_heads/5183514109ffa6face065b5b0ba2a5e46ee4e96c/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/egohand_walk_5_heads/5183514109ffa6face065b5b0ba2a5e46ee4e96c/1200w.webp">
</picture>

### 6. Metric hand mesh

[SOMA](https://arxiv.org/abs/2603.16858) turns pose and shape into a 2,859-vertex mesh per hand, in metres in the camera frame.

<picture>
  <img src="https://static.rerun.io/egohand_walk_6_mesh/46d27cffbc79afe08546723f829df95497fcc497/full.webp" alt="Metric hand mesh">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/egohand_walk_6_mesh/46d27cffbc79afe08546723f829df95497fcc497/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/egohand_walk_6_mesh/46d27cffbc79afe08546723f829df95497fcc497/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/egohand_walk_6_mesh/46d27cffbc79afe08546723f829df95497fcc497/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/egohand_walk_6_mesh/46d27cffbc79afe08546723f829df95497fcc497/1200w.webp">
</picture>

### 7. Output

Boxes, 2D joints on the video, 3D joints and meshes, logged to Rerun for every frame.

<picture>
  <img src="https://static.rerun.io/egohand_walk_7_output/e5f16bf9ed5df680a185b16acb2d33925d0046e7/full.webp" alt="Output">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/egohand_walk_7_output/e5f16bf9ed5df680a185b16acb2d33925d0046e7/480w.webp">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/egohand_walk_7_output/e5f16bf9ed5df680a185b16acb2d33925d0046e7/768w.webp">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/egohand_walk_7_output/e5f16bf9ed5df680a185b16acb2d33925d0046e7/1024w.webp">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/egohand_walk_7_output/e5f16bf9ed5df680a185b16acb2d33925d0046e7/1200w.webp">
</picture>

## How it works

EgoHand is a single-camera model: DINOv3 features with ray encoding feed a 3D hand decoder (SOMA hand pose, [MHR](https://arxiv.org/abs/2511.15586) shape, depth) and a 2D joint decoder, and a closed-form solve places the wrist.
It was trained on about 14 M images.
The fast path keeps the same outputs: it runs the detector and the backbone as TensorRT engines (NVFP4 and FP8 on Blackwell GPUs), crops on the GPU and replays CUDA graphs.

## Run the code

This is an external example in the [posekit package](https://github.com/rerun-io/examples-monorepo/tree/main/packages/posekit) of the Rerun examples monorepo.
It needs Linux (glibc 2.35 or newer) with an NVIDIA GPU (CUDA 13 driver) and about 30 GB of disk.

### With a coding agent

Paste this prompt into any coding agent, or into the agent panel of the Rerun viewer.
It checks your machine, asks before it installs anything, and runs the example.

```agent-prompt
Install and run the Rerun EgoHand example on this machine: NVIDIA's EgoHand finds both hands in a head-camera video and logs 3D joints and hand meshes to the Rerun viewer. Do the steps in order and stop at the first that fails.

1. Check the machine. This example needs Linux (x86_64 or aarch64) with an NVIDIA GPU and a driver for CUDA 13 or newer. Run `uname -sm`, `nvidia-smi` and `ldd --version`. If it is not Linux, `nvidia-smi` is missing, the "CUDA Version" it prints is below 13.0, or glibc is older than 2.35, stop: tell me why, and that I can try the example in the browser at https://pablovela5620-egohand-rerun.hf.space instead.
2. Check Pixi with `pixi --version`; it must be 0.83 or newer. Use Pixi for everything; do not use pip, uv or conda.
3. Choose where the code goes. Propose `~/rerun-examples/examples-monorepo` (not /tmp: the environment and model weights need about 30 GB and must survive a reboot). Run `df -h` on its nearest existing parent (e.g. `df -h ~`), because the folder does not exist yet. Then ask me ONE question that covers everything: the folder, the free space there, and, if needed, that you will install Pixi (`curl -fsSL https://pixi.sh/install.sh | sh`) or update it (`pixi self-update`). Wait for my answer, and use another folder if I name one. If less than 30 GB is free there, say so in the question.
4. Get the code. If the folder already holds a clone of https://github.com/rerun-io/examples-monorepo, run `git pull` in it; otherwise `git clone --depth 1 https://github.com/rerun-io/examples-monorepo.git <folder>`. Then `cd <folder>`. If you installed Pixi, open a new shell (or source the shell rc) so `pixi` is on PATH.
5. Pick where the output goes, then run. Base command: `pixi run --frozen -e posekit-hands posekit-video-hands --video-path assets/examples/genhumanego_kettle.mp4 <output flags> egohand onnx egohand` (the output flags go before `egohand onnx egohand`). The first run installs a large CUDA environment and downloads public weights from Hugging Face, so it takes a while.
   - If you have Rerun viewer tools (you run inside the Rerun viewer's agent panel): send the data to that viewer. Call `rerun_connect` with no endpoint; its reply names the viewer's address, e.g. `http://127.0.0.1:41881`. Use `--rr-config.connect --rr-config.port <that port>`. The video, hand boxes, 2D joints and 3D hands then appear in the viewer you are in.
   - Otherwise, with a display: no output flags. The command spawns a Rerun viewer on port 9876. If a Rerun viewer already listens there, the data shows in that viewer. If something else listens there (`ss -ltnp | grep ':9876 '`), for example a `rerun server`, it swallows the data and you see nothing: then add `--rr-config.port <a free port>`.
   - Otherwise, with no display (for example over SSH, when `DISPLAY` and `WAYLAND_DISPLAY` are both empty): `--rr-config.save <folder>/egohand.rrd` with an absolute path (the task runs in `packages/posekit`, so a relative path lands there), and tell me to open that file with the Rerun viewer on a machine with a screen (`pixi run --frozen -e posekit-hands rerun <folder>/egohand.rrd`, or https://rerun.io/viewer).
6. Check that the data arrived: the output must not contain `Failed to flush sink` or `connection severed`. If it does, the port belongs to something that is not a viewer; fix the port as in step 5 and run again. Inside the viewer's agent panel, also call `rerun_get_state` and check that a `video_hands` recording with a `frame` timeline is open.
7. When it finishes, tell me what ran, how long it took, the folder and its size on disk (`du -sh <folder>`), that deleting that folder removes the code and its environment (the downloaded packages stay in Pixi's cache and the model weights in `~/.cache/huggingface`, where other projects can reuse them), and that I can pass `--video-path <my video.mp4>` to run my own clip.
```

### Manually

Install [Pixi](https://pixi.sh/latest/#installation) 0.83 or newer, then:

```sh
git clone --depth 1 https://github.com/rerun-io/examples-monorepo.git
cd examples-monorepo
pixi run --frozen -e posekit-hands posekit-video-hands --video-path assets/examples/genhumanego_kettle.mp4 egohand onnx egohand
```

The first run installs the environment and downloads the model weights.
It opens the Rerun viewer on a 6 s Gen-HumanEgo clip, already remapped from the fisheye into a 65° pinhole view, so the default intrinsics are exact.
To run your own video, change `--video-path`, and pass `--fx/--fy/--cx/--cy` (or `--hfov-deg`): the 3D placement depends on them.

This runs the reference stack, which works on any supported GPU.
For the fast path in the table above, replace `egohand onnx egohand` with `egohand-fast tensorrt egohand-fast` on a Blackwell GPU, or with `egohand-fast-bf16 tensorrt egohand-fast-bf16` on older GPUs.
The first fast run builds its TensorRT engines, which takes a few minutes.

## Papers and credits

* [EgoHand: Monocular 3D Hand Reconstruction from Egocentric Video](https://arxiv.org/abs/2610.30293), Chen et al. (NVIDIA), with [code](https://github.com/NVIDIA/IsaacCapture/tree/main/src/python/isaaccapture_egopost) and [weights](https://huggingface.co/nvidia/EgoHand-v1).
* [RT-DETR](https://arxiv.org/abs/2304.08069), Zhao et al., and [RT-DETRv2](https://arxiv.org/abs/2407.17140), Lv et al.
* [DINOv3](https://arxiv.org/abs/2508.10104), Siméoni et al. (Meta).
* [MHR: Momentum Human Rig](https://arxiv.org/abs/2511.15586), Ferguson et al. (Meta).
* [SOMA: Unifying Parametric Human Body Models](https://arxiv.org/abs/2603.16858), Saito et al. (NVIDIA), with [SOMA-X](https://github.com/NVlabs/SOMA-X).
* [The Double Sphere Camera Model](https://arxiv.org/abs/1807.08957), Usenko, Demmel and Cremers.
* Weights: [EgoHand-v1](https://huggingface.co/nvidia/EgoHand-v1) and [RT-DETR-Hand-Detector-v1](https://huggingface.co/nvidia/RT-DETR-Hand-Detector-v1) (NVIDIA, OpenMDW-1.1; the example loads EgoHand from a weights-only mirror, [pablovela5620/egohand](https://huggingface.co/pablovela5620/egohand)), and [SOMA-X](https://huggingface.co/nvidia/SOMA-X) (Apache-2.0).
* Video: [Gen-HumanEgo](https://huggingface.co/datasets/genrobot2025/Gen-HumanEgo) ([CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/)). The video, images and recording on this page overlay boxes, joints and hand meshes on it, and are shared under the same licence.
