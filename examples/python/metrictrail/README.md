<!--[metadata]
title = "MetricTrail: metric camera tracking from one video"
description = "Track a moving camera through one uncalibrated video with NVIDIA's MetricTrail and get the camera path, the focal length and a surface in meters, step by step."
tags = ["2D", "3D", "SLAM", "Depth", "Pinhole camera", "Video", "Mesh", "Hugging Face"]
source = "https://github.com/rerun-io/examples-monorepo/tree/main/packages/metrictrail"
thumbnail = "https://pablos-4800gt.ilish-ruler.ts.net:8768/metrictrail-example/media/thumbnail-768x576-8a77f49cf.png"  # PREVIEW ONLY: TODO(pablo) upload to static.rerun.io (pixi run upload-image)
thumbnail_dimensions = [768, 576]
preview_rrd_url = "https://pablo-dl-server.ilish-ruler.ts.net:8477/metrictrail-preview/metrictrail-pool-live-8a77f49cf.rrd"  # PREVIEW ONLY: TODO(pablo) host on a public Hugging Face dataset with a card
-->

[![Open in Hugging Face Spaces](https://huggingface.co/datasets/huggingface/badges/resolve/main/open-in-hf-spaces-md.svg)](https://pablovela5620-metrictrail-rerun.hf.space)

```agent-prompt
```

Try it in your browser with no install (the Space runs MetricTrail on ZeroGPU), or copy a prompt for your coding agent to install and run it on your own Linux machine with an NVIDIA GPU.

<!-- PREVIEW ONLY: media on the tailnet NAS. TODO(pablo): upload to static.rerun.io -->

<video src="https://pablos-4800gt.ilish-ruler.ts.net:8768/metrictrail-example/media/seg-run-8a77f49cf.mp4" poster="https://pablos-4800gt.ilish-ruler.ts.net:8768/metrictrail-example/media/seg-run-8a77f49cf.webp" autoplay muted loop playsinline></video>

MetricTrail tracks a moving camera through one uncalibrated video and returns the camera path, the focal length and a surface, all in meters.
This example runs [NVIDIA's MetricTrail](https://research.nvidia.com/labs/sil/projects/metrictrail/) on a phone video and logs every step to Rerun: the video with the tracked patches, the metric depth, the camera path and a mesh of the scene.

## The result

The video above is 23 s of a phone video on a rooftop terrace, with no calibration given.
Left, top: the video. The amber boxes are the 64 patches of the newest keyframe: small image squares that the DPVO tracker follows from frame to frame.
Left, bottom: the predicted metric depth, 0.5 m (blue) to 20 m (red).
Right: the final camera path and a mesh of the terrace, with the camera moving along the path.
MetricTrail tracks all 690 frames, finds the focal length from the clip itself, and fuses the depth of every frame into one mesh of 1.18 million triangles.
This run uses the open pull requests #579 and #582 to [#585](https://github.com/rerun-io/examples-monorepo/pull/585) of the examples-monorepo.

The same 690 frames on an NVIDIA RTX 5090, from the video file to the mesh (timed on a quiet machine at commit 8148306a5; the pictures come from commit 8a77f49cf, which changes only how patches are drawn):

| Pass | Total | Calibration | Tracking | Speed |
|---|---|---|---|---|
| With capture | 39.95 s | 9.90 s | 25.48 s | 27.1 fps |
| No capture | 27.84 s | 10.18 s | 10.95 s | 63.0 fps |

Mesh fusion takes 2.30 s and 1.98 s.

Where the time goes, per stage: in each pair, the left bar is with capture and the right bar is without. Without capture, the run logs the saved depth maps after tracking, which takes 2.7 s. Other pipeline work is model loading and setup.

<!-- PREVIEW ONLY: media on the tailnet NAS. TODO(pablo): upload to static.rerun.io -->

![Runtime per stage of this run as one Rerun bar chart, with and without capture](https://pablos-4800gt.ilish-ruler.ts.net:8768/metrictrail-example/media/seg-stages-v5.webp)

## How one frame goes through it

Frame 345 of the run above, 11.5 s into the clip: the camera walks along the pool, with the deck on the left and the city behind the glass railing.

<!-- PREVIEW ONLY: media on the tailnet NAS. TODO(pablo): upload to static.rerun.io -->

![The pipeline on frame 345 as a graph: the input frame feeds calibration, image features and metric depth; calibration gives the focal length to metric depth; features and depth feed the patches; patches carry MoGe-2's depth as their prior into bundle adjustment, which solves 10 cameras and 1,408 patch depths and gives the poses of the output; the output mesh also fuses the depth](https://pablos-4800gt.ilish-ruler.ts.net:8768/metrictrail-example/media/metrictrail-flow-dag-v3.webp)

### 1. The input frame

Frame 345 is one frame of the 23 s cut, scaled from the 1920 × 1080 phone video to 1280 × 720, with no calibration given; the tracker reads a copy scaled to 504 × 280. Right: the camera of this frame (white) at its final pose on the path, over the mesh, with a grid of 1 m squares on the deck.

<!-- PREVIEW ONLY: media on the tailnet NAS. TODO(pablo): upload to static.rerun.io -->

![Frame 345 next to its camera on the final path over the mesh, with a 1 m grid on the deck](https://pablos-4800gt.ilish-ruler.ts.net:8768/metrictrail-example/media/walk-1-frame-v4.webp)

### 2. One focal length for the clip

MoGe-2's depth and every camera pose need the focal length that step 1 lacks, so before tracking SharedK fits one camera matrix for the whole clip. MoGe-2 guesses fx and fy on each of the first 32 frames (gray dots: fx, 355.2 to 395.0 px), and SharedK starts from their medians, fx = 375.75 px and fy = 369.33 px. A DROID-style keyframe SLAM on 43 calibration keyframes (left) then moves fx and fy by one shared offset: first down to fx = 374.08 px, then up to fx = 378.60 px (cyan dots: three solves near step 165, two final solves at step 689). The x axis is the calibration step, one per frame of the clip.

The pixels are those of the 504 × 280 tracking image: on the 1280 px frame, fx is about 961.5 px.

<!-- PREVIEW ONLY: media on the tailnet NAS. TODO(pablo): upload to static.rerun.io -->

![The 43-keyframe calibration map next to the focal length after each solve, over the calibration steps](https://pablos-4800gt.ilish-ruler.ts.net:8768/metrictrail-example/media/walk-2-calibration-v4.webp)

### 3. Image features

[DINOv3](https://arxiv.org/abs/2508.10104) turns the frame into an 18 × 32 grid of feature vectors (the flow picture above shows them). A small trained adapter adds them to the features that DPVO uses to match patches (step 5). DPVO's own features describe small local texture, and DINOv3 describes larger regions. The adapter starts from zero in training, so it begins as plain DPVO and learns only what DINOv3 adds.

From left, the picture shows frame 345, DPVO's own features, the DINOv3 term, and their sum. Colors show the three strongest directions of each feature map (PCA) as RGB. DPVO's own features and the sum use the same PCA. Per cell, the DINOv3 term is a median 9 % of the length of DPVO's own feature vector, so the two look almost the same.

<!-- PREVIEW ONLY: media on the tailnet NAS. TODO(pablo): upload to static.rerun.io -->

![Frame 345, DPVO's own matching features, the DINOv3 adapter term and their sum, as PCA colors](https://pablos-4800gt.ilish-ruler.ts.net:8768/metrictrail-example/media/walk-3-fusion-v3.webp)

### 4. Metric depth and normals

[MoGe-2](https://arxiv.org/abs/2507.02546) gives every pixel a depth in meters. Later steps use it: DPVO starts its patches at this depth, and bundle adjustment pulls the path toward it, which fixes the scale of the path.

MoGe-2 has its own encoder, a DINOv2 ViT-L, separate from the DINOv3 of step 3. The encoder turns the frame, resized to 644 × 364, into a 26 × 46 grid of tokens. Image-filter (conv) heads read the tokens and predict a 3D point map with no unit and a mask that drops pixels with no usable depth (0.01 % here). A small MLP reads one summary vector of the whole image (the CLS token) and predicts one scale, s = 5.19. MoGe-2 adds a small shift (-0.05) to z: the offset with which the points, projected through the given focal length, land back on their own pixels. Then it multiplies them by s: the result is points in meters.

<!-- PREVIEW ONLY: media on the tailnet NAS. TODO(pablo): upload to static.rerun.io -->

![Inside MoGe-2: frame, DINOv2 encoder, point map, mask and scale heads, metric points, metric depth](https://pablos-4800gt.ilish-ruler.ts.net:8768/metrictrail-example/media/readme-walk-4-flow-v3.webp)

<!-- PREVIEW ONLY: media on the tailnet NAS. TODO(pablo): upload to static.rerun.io -->

![Frame 345's DINOv2 tokens, the point map's z with no unit (0.1 purple to 3.9 yellow), and the metric points with the camera and a 1 m grid](https://pablos-4800gt.ilish-ruler.ts.net:8768/metrictrail-example/media/readme-walk-4-inside-v2.webp)

The result for frame 345, with depth from 0.5 m (blue) to 20 m (red): 2.6 m to the nearest point and 6.6 m median. On this frame alone, MoGe-2 guesses a focal length of 413.4 px, and its guesses change from frame to frame (step 2). So the example gives it the one SharedK focal length, 378.60 px on the 504 × 280 tracking image. With its own guess, the median depth would be 7.0 m.

The same pass also gives surface normals (color = direction: the deck, which faces up, is magenta). The tracker does not use them; the recording keeps them for display.

<!-- PREVIEW ONLY: media on the tailnet NAS. TODO(pablo): upload to static.rerun.io -->

![Frame 345, its metric depth (0.5 m blue to 20 m red), its surface normals, and the depth key](https://pablos-4800gt.ilish-ruler.ts.net:8768/metrictrail-example/media/readme-walk-4-output-v2.webp)

### 5. Patches start at the predicted depth

DPVO follows the frame through 64 patches. A patch is a 12 x 12 px square, 3 x 3 cells of DPVO's 4 px feature grid, on the 504 x 280 tracking frame. Each patch starts at MoGe-2's depth at its center: all 64 get a valid depth, with a median of 8.0 m (7.9 m after the frame's first bundle adjustment). DPVO draws the 64 centers at random, so their median need not match the 6.6 m median of all pixels. Plain DPVO has no metric depth: it gives all new patches one shared value, the median depth of the patches of the last three keyframes. Below, each box is colored by the depth it starts at, on the same scale as the depth map: 0.5 m (blue) to 20 m (red).

<!-- PREVIEW ONLY: media on the tailnet NAS. TODO(pablo): upload to static.rerun.io -->

![Left: the 64 patches colored by their start depth. Right: MoGe-2 depth with the same boxes. Both use 0.5 m (blue) to 20 m (red)](https://pablos-4800gt.ilish-ruler.ts.net:8768/metrictrail-example/media/walk-5-patches-v2.webp)

The start depth and the camera pose put each patch into the other keyframes. DPVO compares the patch's features only in a 7 x 7-cell window around that spot, so a good start depth starts the search near the answer. The recurrent update then predicts where the patch really lies (its target) and how much to trust that match (a weight from 0 to 1). The update runs again on every new frame. For the 64 patches matched into the older keyframe 342, the median weight rises from 0.10 after the first update to 0.75 after the third, two frames later. Below, purple boxes have a low weight and green to yellow boxes a high one.

<!-- PREVIEW ONLY: media on the tailnet NAS. TODO(pablo): upload to static.rerun.io -->

![Eight patches of frame 345 and their targets in frame 342 after update 1 and update 3, colored by weight from 0 (purple) to 1 (yellow)](https://pablos-4800gt.ilish-ruler.ts.net:8768/metrictrail-example/media/walk-5-tracking-v3.webp)

### 6. Bundle adjustment in meters

At frame 345, DPVO's graph holds 22 keyframes: the ones whose patches still have matches. Bundle adjustment moves the newest 10 cameras and the depths of all 1,408 patches in the graph, so that each patch lands closer to the target that DPVO's network set in step 5. The 12 older cameras stay fixed as anchors. The metric depth term adds one pull per patch toward MoGe-2's depth, and that keeps the scale in meters.

<!-- PREVIEW ONLY: media on the tailnet NAS. TODO(pablo): upload to static.rerun.io -->

![The bundle adjustment at frame 345 from above and from the side: window keyframes 1 to 10 are solved, 12 older keyframes stay fixed, on a 1 m grid](https://pablos-4800gt.ilish-ruler.ts.net:8768/metrictrail-example/media/walk-6-window-v2.webp)

Amber frustums (camera pyramids) 1 to 10 are solved, gray ones stay fixed, and frame 345 is white. Keyframes 7 to 10 are frames 342 to 345: DPVO adds every frame as a keyframe and later drops the ones that add little motion. Blue points belong to the 640 patches of the window keyframes, green points to the 768 patches of the older keyframes. Bundle adjustment solves the depths of both.

<!-- PREVIEW ONLY: media on the tailnet NAS. TODO(pablo): upload to static.rerun.io -->

![Window patches at frame 345, MoGe-2 depth against bundle adjustment depth: with the metric depth term the points follow the equal line; with its weight at 0 they fall below it](https://pablos-4800gt.ilish-ruler.ts.net:8768/metrictrail-example/media/walk-6-prior-v2.webp)

Each point above is one window patch at frame 345: MoGe-2's depth across, the bundle adjustment depth up. With the term, the median gap is 5.5 % (0.42 m). Track the whole clip with the term's weight at 0, and most depths shrink, while some far ones drift past 20 m onto the edge: the median gap grows to 33 % (2.1 m). The reprojection error (how far a patch lands from its target) stays almost the same, 1.04 px with the term and 1.06 px without, so the matches alone cannot fix the scale.

<!-- PREVIEW ONLY: media on the tailnet NAS. TODO(pablo): upload to static.rerun.io -->

![The final keyframe path from above, with the metric depth term (amber, 28.8 m) and with its weight at 0 (purple, 24.5 DPVO units)](https://pablos-4800gt.ilish-ruler.ts.net:8768/metrictrail-example/media/walk-6-noprior-v2.webp)

The path shrinks too: 24.5 DPVO units against 28.8 m with the term, 15 % shorter. The clip has no ground truth, so this shows scale drift, not which length is right. MetricTrail rejects a weight-0 run at its end; this path comes from the tracker before that check.

### 7. Output

The run returns 690 camera poses in meters and a mesh of 1,181,157 triangles, fused from MoGe-2's depth at the final poses (the fusion keeps depths from 0.2 to 8 m, so the far city is not in the mesh). A known size checks the scale: the camera rides about 1.5 m above the deck (1.2 to 1.7 m along the path), the height at which a person holds a phone. The grid squares are 1 m and help to read such sizes. Right: the same mesh colored by its normals, so the flat deck and the walls stand out.

<!-- PREVIEW ONLY: media on the tailnet NAS. TODO(pablo): upload to static.rerun.io -->

![The mesh with the final path, the camera of frame 345 and a 1 m grid, and the same mesh colored by its normals](https://pablos-4800gt.ilish-ruler.ts.net:8768/metrictrail-example/media/walk-7-output-v4.webp)

## How it works

MetricTrail runs these stages on the video:

1. **Calibration (once, before tracking).** SharedK finds one focal length for the whole clip. MoGe-2 gives a first guess on 32 frames, then a DROID-style keyframe SLAM, separate from the tracker, refines it (43 keyframes on this clip). The tracker uses this focal length on every frame.
2. **Features and depth (every frame).** A frozen DINOv3 model and MoGe-2 both read the frame; neither needs the other. A small trained adapter adds the DINOv3 features to the features that DPVO uses to match patches. MoGe-2 predicts metric depth and surface normals in one TensorRT pass.
3. **Patches.** DPVO samples 64 patches per frame. Each patch starts at MoGe-2's depth at that pixel, not at one shared guess for all patches (plain DPVO uses the median depth of the patches of the last three keyframes), and keeps that depth as its prior.
4. **Bundle adjustment.** A recurrent update matches each patch in recent keyframes and gives a target and a weight. Over a small window of recent keyframes, bundle adjustment adds a soft term that pulls each patch depth toward its prior. This keeps the camera path in meters.
5. **Surface.** This example fuses the depth of every tracked frame at its final pose into a TSDF with 4 cm voxels (in Rust), and marching cubes turns it into one mesh.

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
