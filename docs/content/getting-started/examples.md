---
title: Examples
order: 6
---

[//impl ticket]: (https://github.com/rerun-io/rerun/issues/1045)

In the Rerun [GitHub](https://github.com/rerun-io/rerun) repository we maintain a list of examples that demonstrate using the Rerun logging APIs. Generally the examples are individually self-contained, and can be run directly from a Git clone of the repository. Many of the Python examples need additional dependencies set up in a `requirements.txt` next to the example. These are noted in the individual example sections below.

## Setup

Make sure you have the Rerun repository checked out and the latest SDK installed.

```bash
pip install --upgrade rerun-sdk  # install the latest Rerun SDK
git clone git@github.com:rerun-io/rerun.git  # Clone the repository
cd rerun 
git checkout latest  # Check out the commit matching the latest SDK release
```
> Note: Make sure your SDK version matches the examples.
For example, if your SDK version is `0.3.1`, check out the matching tag
in the Rerun repository by running `git checkout v0.3.1`.

## Minimal example

[Python](https://github.com/rerun-io/rerun/tree/latest/examples/python/minimal/main.py) | [Rust](https://github.com/rerun-io/rerun/tree/latest/examples/rust/minimal/src/main.rs)

The simplest example of how to use Rerun, showing how to log a point cloud.

```bash
python examples/python/minimal/main.py
```

![minimal example>](/docs-media/minimal.png)

-------------------------------------------------------------------

## Examples with Real Data

The following examples illustrate using the Rerun logging SDK with potential real-world (if toy) use cases.  They all require additional data to be downloaded, so an internet connection is needed at least once. The dataset fetching logic is all built into the examples, so no additional steps are needed. In some of the examples such as [Stable Diffusion](#stable-diffusion), the algorithm is run on-line, and may benefit from a GPU-enabled PyTorch machine.


### ARKitScenes

[Python](https://github.com/rerun-io/rerun/blob/latest/examples/python/arkitscenes/main.py)

Visualizes the [ARKitScenes dataset](https://github.com/apple/ARKitScenes/) using the Rerun SDK.
The dataset contains color+depth images, the reconstructed mesh and labeled bounding boxes around furniture.

![arkitscenes](/docs-media/arkitscenes.png)

### COLMAP

[Python](https://github.com/rerun-io/rerun/tree/latest/examples/python/colmap/main.py)

![colmap example>](/docs-media/colmap1.png)

An example using Rerun to log and visualize the output of COLMAP's sparse reconstruction.

[COLMAP](https://colmap.github.io/index.html) is a general-purpose Structure-from-Motion (SfM) and Multi-View Stereo (MVS) pipeline with a graphical and command-line interface.

In this example a short video clip has been processed offline by the COLMAP pipeline, and we use Rerun to visualize the individual camera frames, estimated camera poses, and resulting point clouds over time.


```bash
pip install -r examples/python/colmap/requirements.txt
python examples/python/colmap/main.py
```
---

### Deep SDF

[Python](https://github.com/rerun-io/rerun/tree/latest/examples/python/deep_sdf/main.py)

![deep_sdf example>](/docs-media/deep_sdf1.png)

Generate Signed Distance Fields for arbitrary meshes using both traditional methods as well as the one described in the [DeepSDF paper](https://arxiv.org/abs/1901.05103), and visualize the results using the Rerun SDK.

```bash
pip install -r examples/python/deep_sdf/requirements.txt
python examples/python/deep_sdf/main.py
```
---

### Dicom

[Python](https://github.com/rerun-io/rerun/tree/latest/examples/python/dicom/main.py)

![dicom example>](/docs-media/dicom1.png)

Example using a [DICOM](https://en.wikipedia.org/wiki/DICOM) MRI scan. This demonstrates the flexible tensor slicing capabilities of the Rerun viewer.

```bash
pip install -r examples/python/dicom/requirements.txt
python examples/python/dicom/main.py
```
---

### MP Pose

[Python](https://github.com/rerun-io/rerun/tree/latest/examples/python/mp_pose/main.py)

![mp_pose example>](/docs-media/mp_pose1.png)

Use the [MediaPipe](https://google.github.io/mediapipe/) Pose solution to detect and track a human pose in video.

```bash
pip install -r examples/python/mp_pose/requirements.txt
python examples/python/mp_pose/main.py
```
---

### NYUD

[Python](https://github.com/rerun-io/rerun/tree/latest/examples/python/nyud/main.py)

![nyud example>](/docs-media/nyud1.png)

Example using an [example dataset](https://cs.nyu.edu/~silberman/datasets/nyu_depth_v2.html) from New York University with RGB and Depth channels.

```bash
pip install -r examples/python/nyud/requirements.txt
python examples/python/nyud/main.py
```
---

### Objectron

[Python](https://github.com/rerun-io/rerun/tree/latest/examples/python/objectron/main.py) | [Rust](https://github.com/rerun-io/rerun/tree/latest/examples/rust/objectron/src/main.rs)

![objectron example>](/docs-media/objectron1.png)

Example of using the Rerun SDK to log the [Objectron](https://github.com/google-research-datasets/Objectron) dataset.

> The Objectron dataset is a collection of short, object-centric video clips, which are accompanied by AR session metadata that includes camera poses, sparse point-clouds and characterization of the planar surfaces in the surrounding environment.

```bash
pip install -r examples/python/objectron/requirements.txt
python examples/python/objectron/main.py
```
---

### Raw Mesh

[Python](https://github.com/rerun-io/rerun/tree/latest/examples/python/raw_mesh/main.py) | [Rust](https://github.com/rerun-io/rerun/tree/latest/examples/rust/raw_mesh/src/main.rs)

![raw_mesh example>](/docs-media/raw_mesh1.png)

This example demonstrates how to use the Rerun SDK to log raw 3D meshes (so-called "triangle soups") and their transform hierarchy. Simple material properties are supported.

```bash
pip install -r examples/python/raw_mesh/requirements.txt
python examples/python/raw_mesh/main.py
```
---

### Segment Anything

[Python](https://github.com/rerun-io/rerun/tree/latest/examples/python/segment_anything/main.py)

![stable_diffusion example>](/docs-media/segment_anything1.png)

Example of using Rerun to log and visualize the output of Meta AI's Segment Anything model.

For more info see [here](https://segment-anything.com/).

```bash
pip install -r examples/python/segment_anything/requirements.txt
python examples/python/segment_anything/main.py
```
---

### Stable Diffusion

[Python](https://github.com/rerun-io/rerun/tree/latest/examples/python/stable_diffusion/main.py)

![stable_diffusion example>](/docs-media/stable_diffusion1.png)

A more elaborate example running Depth Guided Stable Diffusion 2.0.

For more info see [here](https://github.com/Stability-AI/stablediffusion).

```bash
pip install -r examples/python/stable_diffusion/requirements.txt
python examples/python/stable_diffusion/main.py
```
---

### Tracking HF OpenCV

[Python](https://github.com/rerun-io/rerun/tree/latest/examples/python/tracking_hf_opencv/main.py)

![tracking_hf_opencv example>](/docs-media/tracking_hf_opencv1.png)

Another more elaborate example applying simple object detection and segmentation on a video using the Huggingface `transformers` library. Tracking across frames is performed using [CSRT](https://arxiv.org/pdf/1611.08461.pdf) from OpenCV.

For more info see: https://huggingface.co/docs/transformers/index

```bash
pip install -r examples/python/tracking_hf_opencv/requirements.txt
python examples/python/tracking_hf_opencv/main.py
```

-------------------------------------------------------

## Examples with Artificial Data

The following examples serve to illustrate various uses of the Rerun logging SDK. They should not require any additional data downloads, and should run offline.

### API Demo

[Python](https://github.com/rerun-io/rerun/tree/latest/examples/python/api_demo/main.py) | [Rust](https://github.com/rerun-io/rerun/tree/latest/examples/rust/api_demo/src/main.rs)

![api_demo example>](/docs-media/api_demo1.png)

This is a swiss-army-knife example showing the usage of most of the Rerun SDK APIs. The data logged is static and meaningless.

Multiple sub-examples are available (See the instructions by running with the `--help` flag).

```bash
python examples/python/api_demo/main.py
```
---

### Car

[Python](https://github.com/rerun-io/rerun/tree/latest/examples/python/car/main.py)

![car example>](/docs-media/car1.png)

A very simple 2D car is drawn using OpenCV, and a depth image is simulated and logged as a point cloud.

```bash
pip install -r examples/python/car/requirements.txt
python examples/python/car/main.py
```
---

### Clock

[Python](https://github.com/rerun-io/rerun/tree/latest/examples/python/clock/main.py)

![clock example>](/docs-media/clock1.png)

An example visualizing an analog clock with hour, minute and seconds hands using Rerun Arrow3D primitives.

```bash
python examples/python/clock/main.py
```
---

### Multiprocessing

[Python](https://github.com/rerun-io/rerun/tree/latest/examples/python/multiprocessing/main.py)

Demonstrates how rerun can work with the python `multiprocessing` library.

```bash
python examples/python/multiprocessing/main.py
```
---

### Multithreading

[Python](https://github.com/rerun-io/rerun/tree/latest/examples/python/multithreading/main.py)

Demonstration of logging to Rerun from multiple threads.

```bash
python examples/python/multithreading/main.py
```
---

### Plots

[Python](https://github.com/rerun-io/rerun/tree/latest/examples/python/plots/main.py)

![plots example>](/docs-media/plots1.png)

This example demonstrates how to log simple plots with the Rerun SDK. Charts can be created from 1-dimensional tensors, or from time-varying scalars.

```bash
python examples/python/plots/main.py
```
---

### Text Logging

[Python](https://github.com/rerun-io/rerun/tree/latest/examples/python/text_logging/main.py)

![text_logging example>](/docs-media/text_logging1.png)

This example demonstrates how to integrate python's native `logging` with the Rerun SDK.

Rerun is able to act as a Python logging handler, and can show all your Python log messages in the viewer next to your other data.

```bash
python examples/python/text_logging/main.py
```
----------------------------------------------------------------------
