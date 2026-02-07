<!--[metadata]
title = "Structure from motion"
tags = ["2D", "3D", "COLMAP", "Pinhole camera", "Time series"]
thumbnail = "https://static.rerun.io/structure-from-motion/af24e5e8961f46a9c10399dbc31b6611eea563b4/480w.png"
thumbnail_dimensions = [480, 480]
channel = "main"
include_in_manifest = true
build_args = ["--dataset=colmap_fiat", "--resize=800x600"]
-->

Visualize a sparse reconstruction by [COLMAP](https://colmap.github.io/index.html), a general-purpose Structure-from-Motion (SfM) and Multi-View Stereo (MVS) pipeline with a graphical and command-line interface

<picture data-inline-viewer="examples/structure_from_motion">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/structure_from_motion/b17f8824291fa1102a4dc2184d13c91f92d2279c/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/structure_from_motion/b17f8824291fa1102a4dc2184d13c91f92d2279c/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/structure_from_motion/b17f8824291fa1102a4dc2184d13c91f92d2279c/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/structure_from_motion/b17f8824291fa1102a4dc2184d13c91f92d2279c/1200w.png">
  <img src="https://static.rerun.io/structure_from_motion/b17f8824291fa1102a4dc2184d13c91f92d2279c/full.png" alt="Structure From Motion example screenshot">
</picture>

## Background

COLMAP is a general-purpose Structure-from-Motion (SfM) and Multi-View Stereo (MVS) pipeline.
In this example, a short video clip has been processed offline using the COLMAP pipeline.
The processed data was then visualized using Rerun, which allowed for the visualization of individual camera frames, estimation of camera poses, and creation of point clouds over time.
By using COLMAP in combination with Rerun, a highly-detailed reconstruction of the scene depicted in the video was generated.

## Used Rerun types

[`Points2D`](https://www.rerun.io/docs/reference/types/archetypes/points2d), [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d), [`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d), [`SeriesLines`](https://www.rerun.io/docs/reference/types/archetypes/series_lines), [`Scalars`](https://www.rerun.io/docs/reference/types/archetypes/scalars), [`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole), [`Image`](https://www.rerun.io/docs/reference/types/archetypes/image), [`TextDocument`](https://www.rerun.io/docs/reference/types/archetypes/text_document)

## Logging and visualizing with Rerun

The visualizations in this example were created with the following Rerun code:

### Timelines

All data logged using Rerun in the following sections is connected to a specific frame.
Rerun assigns a frame id to each piece of logged data, and these frame ids are associated with a [`timeline`](https://www.rerun.io/docs/concepts/timelines).

 ```python
rr.set_time("frame", sequence=frame_idx)
 ```

### Images
The images are logged through the [`Image`](https://www.rerun.io/docs/reference/types/archetypes/image) to the `camera/image` entity.

```python
rr.log("camera/image", rr.Image(rgb).compress(jpeg_quality=75))
```

### Cameras
The images stem from pinhole cameras located in the 3D world. To visualize the images in 3D, the pinhole projection has
to be logged and the camera pose (this is often referred to as the intrinsics and extrinsics of the camera,
respectively).

The [`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole) is logged to the `camera/image` entity and defines the intrinsics of the camera.
This defines how to go from the 3D camera frame to the 2D image plane. The extrinsics are logged as an
[`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d) to the `camera` entity.

```python
rr.log("camera", rr.Transform3D(translation=image.tvec, rotation=rr.Quaternion(xyzw=quat_xyzw), relation=rr.TransformRelation.ChildFromParent))
```

```python
rr.log(
    "camera/image",
    rr.Pinhole(
        resolution=[camera.width, camera.height],
        focal_length=camera.params[:2],
        principal_point=camera.params[2:],
    ),
)
```

### Reprojection error
For each image a [`Scalars`](https://www.rerun.io/docs/reference/types/archetypes/scalars) archetype containing the average reprojection error of the keypoints is logged to the
`plot/avg_reproj_err` entity.

```python
rr.log("plot/avg_reproj_err", rr.Scalars(np.mean(point_errors)))
```

### 2D points
The 2D image points that are used to triangulate the 3D points are visualized by logging as [`Points2D`](https://www.rerun.io/docs/reference/types/archetypes/points2d)
to the `camera/image/keypoints` entity. Note that these keypoints are a child of the
`camera/image` entity, since the points should show in the image plane.

```python
rr.log("camera/image/keypoints", rr.Points2D(visible_xys, colors=[34, 138, 167]))
```

### 3D points
The colored 3D points were added to the visualization by logging the [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d) archetype to the `points` entity.
```python
rr.log("points", rr.Points3D(points, colors=point_colors), rr.AnyValues(error=point_errors))
```

## Run the code
To run this example, make sure you have the Rerun repository checked out and the latest SDK installed:
```bash
pip install --upgrade rerun-sdk  # install the latest Rerun SDK
git clone git@github.com:rerun-io/rerun.git  # Clone the repository
cd rerun
git checkout latest  # Check out the commit matching the latest SDK release
```
Install the necessary libraries specified in the requirements file:
```bash
pip install -e examples/python/structure_from_motion
```
To experiment with the provided example, simply execute the main Python script:
```bash
python -m structure_from_motion # run the example
```
If you wish to customize it, explore additional features, or save it use the CLI with the `--help` option for guidance:
```bash
python -m structure_from_motion --help
```
