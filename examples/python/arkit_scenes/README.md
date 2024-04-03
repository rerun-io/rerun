<!--[metadata]
title = "ARKit Scenes"
tags = ["2D", "3D", "depth", "mesh", "object-detection", "pinhole-camera"]
description = "This example visualizes the ARKitScenes dataset using Rerun. The dataset contains color images, depth images, the reconstructed mesh, and labeled bounding boxes around furniture."
thumbnail = "https://static.rerun.io/arkit-scenes/6d920eaa42fb86cfd264d47180ecbecbb6dd3e09/480w.png"
thumbnail_dimensions = [480, 480]
channel = "main"
-->


<picture data-inline-viewer="examples/arkit_scenes">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/arkit_scenes/fb9ec9e8d965369d39d51b17fc7fc5bae6be10cc/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/arkit_scenes/fb9ec9e8d965369d39d51b17fc7fc5bae6be10cc/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/arkit_scenes/fb9ec9e8d965369d39d51b17fc7fc5bae6be10cc/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/arkit_scenes/fb9ec9e8d965369d39d51b17fc7fc5bae6be10cc/1200w.png">
  <img src="https://static.rerun.io/arkit_scenes/fb9ec9e8d965369d39d51b17fc7fc5bae6be10cc/full.png" alt="ARKit Scenes screenshot">
</picture>

This example visualizes the [ARKitScenes dataset](https://github.com/apple/ARKitScenes/) using Rerun. The dataset
contains color images, depth images, the reconstructed mesh, and labeled bounding boxes around furniture.

## Used Rerun Types
[`Image`](https://www.rerun.io/docs/reference/types/archetypes/image),
[`DepthImage`](https://www.rerun.io/docs/reference/types/archetypes/depth_image), [`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d),
[`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole), [`Mesh3D`](https://www.rerun.io/docs/reference/types/archetypes/mesh3d),
[`Boxes3D`](https://www.rerun.io/docs/reference/types/archetypes/boxes3d),
[`TextDocument`](https://www.rerun.io/docs/reference/types/archetypes/text_document)

## Background

The ARKitScenes dataset, captured using Apple's ARKit technology, encompasses a diverse array of indoor scenes, offering color and depth images, reconstructed 3D meshes, and labeled bounding boxes around objects like furniture. It's a valuable resource for researchers and developers in computer vision and augmented reality, enabling advancements in object recognition, depth estimation, and spatial understanding.

## Logging and Visualizing with Rerun
This visualization through Rerun highlights the dataset's potential in developing immersive AR experiences and enhancing machine learning models for real-world applications while showcasing Reruns visualization capabilities.

# Logging a moving RGB-D camera
To log a moving RGB-D camera, we log four key components: the camera's intrinsics via a pinhole camera model, its pose or extrinsics, along with the color and depth images. The camera intrinsics, which define the camera's lens properties, and the pose, detailing its position and orientation, are logged to create a comprehensive 3D to 2D mapping. Both the RGB and depth images are then logged as child entities, capturing the visual and depth aspects of the scene, respectively. This approach ensures a detailed recording of the camera's viewpoint and the scene it captures, all stored compactly under the same entity path for simplicity.
```python
rr.log("world/camera_lowres", rr.Transform3D(transform=camera_from_world))
rr.log("world/camera_lowres", rr.Pinhole(image_from_camera=intrinsic, resolution=[w, h]))
rr.log(f"{entity_id}/rgb", rr.Image(rgb).compress(jpeg_quality=95))
rr.log(f"{entity_id}/depth", rr.DepthImage(depth, meter=1000))
```

### Ground-truth mesh
The mesh is logged as an [rr.Mesh3D archetype](https://www.rerun.io/docs/reference/types/archetypes/mesh3d).
In this case the mesh is composed of mesh vertices, indices (i.e., which vertices belong to the same face), and vertex
colors.
```python
rr.log(
    "world/mesh",
    rr.Mesh3D(
        vertex_positions=mesh.vertices,
        vertex_colors=mesh.visual.vertex_colors,
        indices=mesh.faces,
    ),
    timeless=True,
)
```
Here, the mesh is logged to the world/mesh entity and is marked as timeless, since it does not change in the context of this visualization.

# Logging 3D bounding boxes
Here we loop through the data and add bounding boxes to all the items found.
```python
for i, label_info in enumerate(annotation["data"]):
    rr.log(
        f"world/annotations/box-{uid}-{label}",
        rr.Boxes3D(
            half_sizes=half_size,
            centers=centroid,
            rotations=rr.Quaternion(xyzw=rot.as_quat()),
            labels=label,
            colors=colors[i],
        ),
        timeless=True,
    )
```
<!--
# Projecting 3D bounding boxes to 2D and logging the line segments
```python
for i, (label, bbox_2d) in enumerate(zip(bbox_labels, bboxes_2d)):
    log_line_segments(f"{entity_id}/bbox-2D-segments/{label}", bbox_2d.reshape(-1, 2), colors[i], label)
```
 -->


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
pip install -r examples/python/arkit_scenes/requirements.txt
```

To run this example use
```bash
python examples/python/arkit_scenes/main.py
```

