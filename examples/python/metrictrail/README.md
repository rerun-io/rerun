<!--[metadata]
title = "MetricTrail: metric camera tracking from one video"
description = "Track a moving camera through one uncalibrated video with NVIDIA's MetricTrail and get the camera path, the focal length and a surface in meters, step by step."
tags = ["2D", "3D", "SLAM", "Depth", "Pinhole camera", "Video", "Mesh", "Hugging Face"]
source = "https://github.com/rerun-io/examples-monorepo/tree/main/packages/metrictrail"
thumbnail = "https://pablos-4800gt.ilish-ruler.ts.net:8768/metrictrail-example/media/thumbnail-768x576.png"  # PREVIEW ONLY: TODO(pablo) upload to static.rerun.io (pixi run upload-image)
thumbnail_dimensions = [768, 576]
preview_rrd_url = "https://pablo-dl-server.ilish-ruler.ts.net:8477/metrictrail-preview/metrictrail-pool-live.rrd"  # PREVIEW ONLY: TODO(pablo) host on a public Hugging Face dataset with a card
-->

[![Open in Hugging Face Spaces](https://huggingface.co/datasets/huggingface/badges/resolve/main/open-in-hf-spaces-md.svg)](https://pablovela5620-metrictrail-rerun.hf.space)

```agent-prompt
```

Try it in your browser with no install (the Space runs MetricTrail on ZeroGPU), or copy a prompt for your coding agent to install and run it on your own Linux machine with an NVIDIA GPU.

<!-- PREVIEW ONLY: media on the tailnet NAS. TODO(pablo): upload to static.rerun.io -->

<video src="https://pablos-4800gt.ilish-ruler.ts.net:8768/metrictrail-example/media/seg-run.mp4" poster="https://pablos-4800gt.ilish-ruler.ts.net:8768/metrictrail-example/media/seg-run.webp" autoplay muted loop playsinline></video>

MetricTrail tracks a moving camera through one uncalibrated video and returns the camera path, the focal length and a surface, all in meters.
This example runs [NVIDIA's MetricTrail](https://research.nvidia.com/labs/sil/projects/metrictrail/) on a phone video and logs every step to Rerun: the video with the tracked patches, the metric depth, the camera path and a mesh of the scene.

## The result

The video above is 23 s of a phone video on a rooftop terrace, with no calibration given.
Left, top: the video with the tracked patches (color = depth, 0.2 m to 6 m); below it, the predicted metric depth (0.5 m to 20 m).
Right: the camera path and a mesh of the terrace, in meters.
MetricTrail tracks all 690 frames, finds the focal length from the clip itself, and fuses the depth of every frame into one mesh of 1.18 million triangles.
This run uses the open pull requests #579 and #582 to [#585](https://github.com/rerun-io/examples-monorepo/pull/585) of the examples-monorepo.

The same 690 frames on an NVIDIA RTX 5090, from the video file to the mesh:

| Pass | Total | Calibration | Tracking | Speed |
|---|---|---|---|---|
| With capture (above) | 39.95 s | 9.90 s | 25.48 s | 27.1 fps |
| No capture | 27.84 s | 10.18 s | 10.95 s | 63.0 fps |

Mesh fusion takes 2.30 s and 1.98 s.

Where the time goes, per stage, with capture (left) and without (right):

<!-- PREVIEW ONLY: media on the tailnet NAS. TODO(pablo): upload to static.rerun.io -->

![Runtime per stage of this run as two Rerun bar charts](https://pablos-4800gt.ilish-ruler.ts.net:8768/metrictrail-example/media/seg-stages-v2.webp)

## How one frame goes through it

Frame 345 of the run above, 11.5 s into the clip: the camera walks along the pool, with the deck on the left and the city behind the glass railing.

<!-- PREVIEW ONLY: media on the tailnet NAS. TODO(pablo): upload to static.rerun.io -->

![The pipeline on frame 345: input frame, calibration, DINOv3 features, metric depth, patches, bundle adjustment, path and mesh](https://pablos-4800gt.ilish-ruler.ts.net:8768/metrictrail-example/media/metrictrail-flow.webp)

### 1. The input frame

Frame 345 is one frame of the 23 s cut (scaled to 1280 × 720), with no calibration given. Right: the same frame on its camera, at its final pose over the mesh.

<!-- PREVIEW ONLY: media on the tailnet NAS. TODO(pablo): upload to static.rerun.io -->

![Frame 345 next to its camera on the final path over the mesh](https://pablos-4800gt.ilish-ruler.ts.net:8768/metrictrail-example/media/walk-1-frame.webp)

### 2. One focal length for the clip

SharedK keeps 43 calibration keyframes and fits one focal length for the whole clip, 378.61 px at the end. MoGe-2's guesses on the first 32 frames (gray dots) spread from 355.2 to 395.0 px.

<!-- PREVIEW ONLY: media on the tailnet NAS. TODO(pablo): upload to static.rerun.io -->

![The 43-keyframe calibration map next to the focal length over the calibration steps](https://pablos-4800gt.ilish-ruler.ts.net:8768/metrictrail-example/media/walk-2-calibration-v2.webp)

### 3. Image features

[DINOv3](https://arxiv.org/abs/2508.10104) turns the frame into one feature vector per image patch. The right view shows three PCA components of these features as color.

<!-- PREVIEW ONLY: media on the tailnet NAS. TODO(pablo): upload to static.rerun.io -->

![Frame 345 next to the PCA colors of its DINOv3 features](https://pablos-4800gt.ilish-ruler.ts.net:8768/metrictrail-example/media/dd-dino-frame-pca.webp)

### 4. Metric depth and normals

MoGe-2 predicts the depth of every pixel in meters: 2.6 m to the nearest point and 6.6 m median (color from 0.5 m, blue, to 20 m, red). Frame 345 is a new keyframe, so the example also logs its surface normals.

<!-- PREVIEW ONLY: media on the tailnet NAS. TODO(pablo): upload to static.rerun.io -->

![Frame 345, its metric depth and its surface normals](https://pablos-4800gt.ilish-ruler.ts.net:8768/metrictrail-example/media/readme-walk-4-depth.webp)

### 5. Patches start at the predicted depth

DPVO samples 64 patches from the frame and starts each one at MoGe-2's depth (right), not at a random depth. Their median depth is 7.9 m. Patch color uses a 0.2 m to 6 m scale, so patches past 6 m are red; the depth on the right uses 0.5 m to 20 m.

<!-- PREVIEW ONLY: media on the tailnet NAS. TODO(pablo): upload to static.rerun.io -->

![The 64 new patches over frame 345 next to its metric depth](https://pablos-4800gt.ilish-ruler.ts.net:8768/metrictrail-example/media/readme-walk-5-patches.webp)

### 6. Bundle adjustment in meters

At this frame the graph holds 22 keyframes, and bundle adjustment solves the newest 10 with their 640 patch points. A soft term keeps each patch depth near MoGe-2's depth, so the path stays in meters.

<!-- PREVIEW ONLY: media on the tailnet NAS. TODO(pablo): upload to static.rerun.io -->

![The bundle adjustment window at frame 345 from the side and from above](https://pablos-4800gt.ilish-ruler.ts.net:8768/metrictrail-example/media/dd-metricba-window.webp)

### 7. Output

The run returns 690 camera poses in meters and a mesh of 1,181,347 triangles; the camera of frame 345 sits on the path. Right: the same mesh colored by its normals, so the flat deck and the walls stand out.

<!-- PREVIEW ONLY: media on the tailnet NAS. TODO(pablo): upload to static.rerun.io -->

![The mesh with the final path and the camera of frame 345, and the same mesh colored by its normals](https://pablos-4800gt.ilish-ruler.ts.net:8768/metrictrail-example/media/readme-walk-7-output.webp)

## How it works

MetricTrail runs these stages on the video, in order:

1. **Calibration.** SharedK finds one focal length for the whole clip. MoGe-2 gives a first guess on 32 frames, then a separate DROID-SLAM keyframe map refines it (43 keyframes on this clip).
2. **Features.** A frozen DINOv3 model adds its features to the features that DPVO uses to match patches, so the matches are more reliable.
3. **Depth and normals.** MoGe-2 predicts metric depth and surface normals for each frame in one TensorRT pass.
4. **Patches.** DPVO samples 64 patches per frame. Each patch starts at MoGe-2's depth, not at a random depth.
5. **Bundle adjustment.** Over a small window of recent keyframes, bundle adjustment adds a soft term that pulls each patch depth toward MoGe-2's depth. This keeps the camera path in meters.
6. **Surface.** This example fuses the depth of every tracked frame at its final pose into a TSDF with 4 cm voxels (in Rust), and marching cubes turns it into one mesh.

For Rerun, the example logs the input video file as a video stream, so the recording keeps the original video samples instead of one image per frame.
It logs the calibration step by step on its own `calibration_step` timeline: the keyframe map, the depth and the focal length as it converges.
One default layout shows the video, depth, normals and features on the left, and tabs for calibration, tracking and the final surface on the right.

## Run the code

This is an external example in the [metrictrail package](https://github.com/rerun-io/examples-monorepo/tree/main/packages/metrictrail) of the Rerun examples monorepo.
It needs Linux (x86_64 or aarch64, glibc 2.35 or newer) with an NVIDIA GPU (CUDA 13 driver) and about 30 GB of disk.

<!-- PREVIEW ONLY: TODO(pablo) remove this sentence when #579, #582 to #585 and #587 are merged -->

The commands below work on `main` only after the open pull requests #579, #582 to #585 and #587 are merged ([#587](https://github.com/rerun-io/examples-monorepo/pull/587) is the last one).

### With a coding agent

Paste this prompt into any coding agent, or into the agent panel of the Rerun viewer.
It checks your machine, asks before it installs anything, and runs the example.

```agent-prompt
Install and run the Rerun MetricTrail example on this machine: NVIDIA's MetricTrail tracks a moving camera through one uncalibrated video and logs the camera path, the focal length, metric depth and a mesh to the Rerun viewer. Do the steps in order and stop at the first that fails.

1. Check the machine. This example needs Linux (x86_64 or aarch64) with an NVIDIA GPU and a driver for CUDA 13 or newer. Run `uname -sm`, `nvidia-smi` and `ldd --version`. If it is not Linux, `nvidia-smi` is missing, the "CUDA Version" it prints is below 13.0, or glibc is older than 2.35, stop: tell me why, and that I can try the example in the browser at https://pablovela5620-metrictrail-rerun.hf.space instead.
2. Check Pixi with `pixi --version`; it must be 0.83 or newer. Use Pixi for everything; do not use pip, uv or conda.
3. Choose where the code goes. Propose `~/rerun-examples/examples-monorepo` (not /tmp: the clone with its environment takes about 18 GB, the model caches about 5 GB more, and both must survive a reboot). Run `df -h` on its nearest existing parent (e.g. `df -h ~`), because the folder does not exist yet. Then ask me ONE question that covers everything: the folder, the free space there, and, if needed, that you will install Pixi (`curl -fsSL https://pixi.sh/install.sh | sh`) or update it (`pixi self-update`). Wait for my answer, and use another folder if I name one. If less than 30 GB is free there, say so in the question.
4. Get the code. If the folder already holds a clone of https://github.com/rerun-io/examples-monorepo, run `git pull` in it; otherwise `git clone --depth 1 https://github.com/rerun-io/examples-monorepo.git <folder>`. Then `cd <folder>`. If you installed Pixi, open a new shell (or source the shell rc) so `pixi` is on PATH.
5. Pick where the output goes, then run. Base command: `pixi run --frozen -e metrictrail metrictrail-example <output flags>`. It tracks the 86 s iPhone clip `pool.mp4`. The first run installs a large CUDA environment, builds three Mojo kernel libraries, downloads the weights (about 120 MB: two files from Hugging Face and droid.pth from Google Drive, plus the 1.3 GB MoGe-2 checkpoint from Hugging Face) and the clip (64 MB), and builds TensorRT engines. On a DGX Spark that already had the conda packages and MoGe-2 cached, this took about 7 minutes; on a clean machine the package and checkpoint downloads add to that. Later runs take about 4 minutes on a DGX Spark and under 2 minutes on an RTX 5090.
   - If you have Rerun viewer tools (you run inside the Rerun viewer's agent panel): send the data to that viewer. Call `rerun_connect` with no endpoint; its reply names the viewer's address, e.g. `http://127.0.0.1:41881`. Use `--rr-config.connect --rr-config.port <that port>`. The video, depth, calibration, camera path and mesh then appear in the viewer you are in.
   - Otherwise, with a display: no output flags. The command spawns a Rerun viewer on port 9876. If a Rerun viewer already listens there, the data shows in that viewer. If something else listens there (`ss -ltnp | grep ':9876 '`), for example a `rerun server`, it swallows the data and you see nothing: then add `--rr-config.port <a free port>`.
   - Otherwise, with no display (for example over SSH, when `DISPLAY` and `WAYLAND_DISPLAY` are both empty): `--rr-config.save <folder>/metrictrail.rrd` with an absolute path (the task runs in `packages/metrictrail`, so a relative path lands there; the file is about 360 MB), and tell me to open that file with the Rerun viewer on a machine with a screen (`pixi run --frozen -e metrictrail rerun <folder>/metrictrail.rrd`, or https://rerun.io/viewer).
6. Check that the data arrived: the output must not contain `Failed to flush sink` or `connection severed`. If it does, the port belongs to something that is not a viewer; fix the port as in step 5 and run again. Inside the viewer's agent panel, also call `rerun_get_state` and check that a recording with the application id `run` and the timelines `video_time` and `calibration_step` is open.
7. When it finishes, tell me what ran, how long it took, the folder and its size on disk (`du -sh <folder>`), and that deleting that folder removes the code, its environment, the weights, the clip and the run outputs. Say that some caches stay, where other projects can reuse them: the downloaded packages in Pixi's cache, MoGe-2 in `~/.cache/huggingface`, the TensorRT engines in `~/.cache/monoprior` and `~/.cache/trtkit`, and `droid.pth` in `~/.cache/torch`. Tell me that I can run my own clip with `pixi run --frozen -e metrictrail metrictrail-run --input <absolute path to my video.mp4> --benchmark video --output <absolute output folder> <output flags>`.
```

### Manually

Install [Pixi](https://pixi.sh/latest/#installation) 0.83 or newer, then:

```sh
git clone --depth 1 https://github.com/rerun-io/examples-monorepo.git
cd examples-monorepo
pixi run --frozen -e metrictrail metrictrail-example
```

The first run installs the environment, builds the Mojo kernels and TensorRT engines, and downloads the weights and the 86 s example clip; with the packages and MoGe-2 already cached, it took about 7 minutes on a DGX Spark.
To run your own video, use `pixi run --frozen -e metrictrail metrictrail-run --input /abs/path/video.mp4 --benchmark video --output /abs/path/out` with absolute paths (the task runs in `packages/metrictrail`).

## Papers and credits

* **MetricTrail**: [paper](https://research.nvidia.com/labs/sil/projects/metrictrail/assets/MetricTrail.pdf) and [project page](https://research.nvidia.com/labs/sil/projects/metrictrail/), Gao et al. (NVIDIA, 2026), with [code](https://github.com/nv-tlabs/vipe) and [weights](https://huggingface.co/nvidia/MetricTrail).
* [ViPE: Video Pose Engine for 3D Geometric Perception](https://arxiv.org/abs/2508.10934), Huang et al. (NVIDIA, 2025), with [code](https://github.com/nv-tlabs/vipe).
* [Deep Patch Visual Odometry](https://arxiv.org/abs/2208.04726) (DPVO), Teed, Lipson and Deng (NeurIPS 2023), with [code](https://github.com/princeton-vl/DPVO).
* [MoGe-2: Accurate Monocular Geometry with Metric Scale and Sharp Details](https://arxiv.org/abs/2507.02546), Wang et al. (NeurIPS 2025), with [code](https://github.com/microsoft/MoGe) and [weights](https://huggingface.co/Ruicheng/moge-2-vitl-normal).
* [DINOv3](https://arxiv.org/abs/2508.10104), Siméoni et al. (Meta, 2025), with [code](https://github.com/facebookresearch/dinov3) and [weights](https://huggingface.co/facebook/dinov3-vits16-pretrain-lvd1689m).
* [DROID-SLAM: Deep Visual SLAM for Monocular, Stereo, and RGB-D Cameras](https://arxiv.org/abs/2108.10869), Teed and Deng (NeurIPS 2021), with [code](https://github.com/princeton-vl/DROID-SLAM). SharedK calibration uses it.
* TSDF fusion: [A Volumetric Method for Building Complex Models from Range Images](https://graphics.stanford.edu/papers/volrange/), Curless and Levoy (SIGGRAPH 1996). Truncation and voxel conventions from [Open3D](https://github.com/isl-org/Open3D) ([paper](https://arxiv.org/abs/1801.09847)); marching-cubes table from [Paul Bourke](https://paulbourke.net/geometry/polygonise/).
