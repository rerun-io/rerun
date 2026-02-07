<!--[metadata]
title = "Objectron"
tags = ["2D", "3D", "Object detection", "Pinhole camera", "Blueprint"]
thumbnail = "https://static.rerun.io/objectron/b645ef3c8eff33fbeaefa6d37e0f9711be15b202/480w.png"
thumbnail_dimensions = [480, 480]
# Channel = "release" - disabled because it sometimes have bad first-frame heuristics
build_args = ["--frames=150"]
-->

Visualize the [Google Research Objectron](https://github.com/google-research-datasets/Objectron) dataset including camera poses, sparse point-clouds and surfaces characterization.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/objectron/8ea3a37e6b4af2e06f8e2ea5e70c1951af67fea8/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/objectron/8ea3a37e6b4af2e06f8e2ea5e70c1951af67fea8/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/objectron/8ea3a37e6b4af2e06f8e2ea5e70c1951af67fea8/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/objectron/8ea3a37e6b4af2e06f8e2ea5e70c1951af67fea8/1200w.png">
  <img src="https://static.rerun.io/objectron/8ea3a37e6b4af2e06f8e2ea5e70c1951af67fea8/full.png" alt="Objectron example screenshot">
</picture>

## Used Rerun types
 [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d), [`Boxes3D`](https://www.rerun.io/docs/reference/types/archetypes/boxes3d), [`EncodedImage`](https://www.rerun.io/docs/reference/types/archetypes/encoded_image), [`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d), [`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole)

## Background

This example visualizes the Objectron database, a rich collection of object-centric video clips accompanied by AR session metadata.
With high-resolution images, object pose, camera pose, point-cloud, and surface plane information available for each sample, the visualization offers a comprehensive view of the object from various angles.
Additionally, the dataset provides manually annotated 3D bounding boxes, enabling precise object localization and orientation.

## Logging and visualizing with Rerun

The visualizations in this example were created with the following Rerun code:

### Timelines

For each processed frame, all data sent to Rerun is associated with the two [`timelines`](https://www.rerun.io/docs/concepts/timelines) `time` and `frame_idx`.

```python
rr.set_time("frame", sequence=sample.index)
rr.set_time("time", duration=sample.timestamp)
```

### Video

Pinhole camera is utilized for achieving a 3D view and camera perspective through the use of the [`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole) and [`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d) archetypes.

```python
rr.log(
        "world/camera",
        rr.Transform3D(translation=translation, rotation=rr.Quaternion(xyzw=rot.as_quat())),
)
```

```python
rr.log(
    "world/camera",
    rr.Pinhole(
        resolution=[w, h],
        image_from_camera=intrinsics,
        camera_xyz=rr.ViewCoordinates.RDF,
    ),
)
```
The input video is logged as a sequence of [`EncodedImage`](https://www.rerun.io/docs/reference/types/archetypes/encoded_image) objects to the `world/camera` entity.
```python
rr.log("world/camera", rr.EncodedImage(path=sample.image_path))
```

### Sparse point clouds

Sparse point clouds from `ARFrame` are logged as [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d) archetype to the `world/points` entity.

```python
rr.log("world/points", rr.Points3D(positions, colors=[255, 255, 255, 255]))
```

### Annotated bounding boxes

Bounding boxes annotated from `ARFrame` are logged as [`Boxes3D`](https://www.rerun.io/docs/reference/types/archetypes/boxes3d), containing details such as object position, sizes, center and rotation.

```python
rr.log(
    f"world/annotations/box-{bbox.id}",
    rr.Boxes3D(
        half_sizes=0.5 * np.array(bbox.scale),
        centers=bbox.translation,
        rotations=rr.Quaternion(xyzw=rot.as_quat()),
        colors=[160, 230, 130, 255],
        labels=bbox.category,
    ),
    static=True,
)
```

### Setting up the default blueprint

The default blueprint is configured with the following code:

```python
blueprint = rrb.Horizontal(
    rrb.Spatial3DView(origin="/world", name="World"),
    rrb.Spatial2DView(origin="/world/camera", name="Camera", contents=["/world/**"]),
)
```

In particular, we want to reproject the points and the 3D annotation box in the 2D camera view corresponding to the pinhole logged at `"/world/camera"`. This is achieved by setting the view's contents to the entire `"/world/**"` subtree, which include both the pinhole transform and the image data, as well as the point cloud and the 3D annotation box.



## Run the code
To run this example, make sure you have the [required Python version](https://ref.rerun.io/docs/python/main/common#supported-python-versions), the Rerun repository checked out and the latest SDK installed:
```bash
pip install --upgrade rerun-sdk  # install the latest Rerun SDK
git clone git@github.com:rerun-io/rerun.git  # Clone the repository
cd rerun
git checkout latest  # Check out the commit matching the latest SDK release
```
Install the necessary libraries specified in the requirements file:
```bash
pip install -e examples/python/objectron
```
To experiment with the provided example, simply execute the main Python script:
```bash
python -m objectron # run the example
```

You can specify the objectron recording:
```bash
python -m objectron --recording {bike,book,bottle,camera,cereal_box,chair,cup,laptop,shoe}
```

If you wish to customize it, explore additional features, or save it use the CLI with the `--help` option for guidance:
```bash
python -m objectron --help
```
