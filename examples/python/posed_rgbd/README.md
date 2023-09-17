---
title: Posed RGB-D Image
thumbnail: https://static.rerun.io/0d2e95315a9eb546cf6eecbc2642a044d044141a_point_tracks_recipe_480w.png
---

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/c7f24fbacc5b61e53c6ff9367d464e99fb46ed06_point_tracks_recipe_header_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/4a12cb85034faa68a7cac2c8bacbc9c372f20c03_point_tracks_recipe_header_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/bf309584ad5490eb4be216fc37bc5e071a00f541_point_tracks_recipe_header_1024w.png">
  <img src="https://static.rerun.io/49b8447fd4aa2b0544aeed978b3f3c0bd33b93c5_point_tracks_recipe_header_full.png" alt="">
</picture>


In this recipe we will show how to log a moving RGB-D image like the one shown above in Rerun. A real-world implementation of the same idea can be found in the [SimpleRecon](/examples/paper-visualizations/simplerecon) example.

We start by generating some synthetic RGB-D images. The idea is simple: a blue cube on top of a grey plane.

```python
# scene parameters
cube_aabb = np.array([[-0.5, -0.5, 0.0], [0.5, 0.5, 1.0]])  # we represent the cube as an axis-aligned bounding box
plane = np.array([[0.0, 0.0, 1.0, 0.0]])  # the plane is represented as a plane equation

# camera parameters
width = 640
height = 480
cx = width / 2.0
cy = height / 2.0
focal_length = 1.0

# sequence parameters
duration = 5.0
num_steps = int(duration * 60)  # 60 FPS
ijs = np.mgrid[:height, :width].reshape(2, -1).T  # ij pairs for each pixel
d_x = (ijs[..., 1] - cx) / focal_length
d_y = (ijs[..., 0] - cy) / focal_length
d_z = np.ones_like(d_x)
```

At this point we have RGBD images as numpy arrays of shape `(num_steps, height, width, 4)`, time steps of shape `(num_steps,)`, camera poses of shape `(num_steps, 4, 4)`, and the width, height and focal length of the camera.

To see these images in Rerun, we need to log them. We can do this by creating a `rr2.Image` object and logging it to Rerun. Note that we need to set the time for each image so that Rerun knows when to display it.



Combining all the previous steps, we get the following code:
```python

```
