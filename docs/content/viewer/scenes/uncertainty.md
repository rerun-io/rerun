---
title: Visualize uncertainty and covariance
order: 500
description: Show covariance ellipsoids in 3D, covariance ellipses in 2D, error bands in plots, and Gaussian splats
---

## Covariance ellipsoids in 3D

Use [`Ellipsoids3D`](../../reference/types/archetypes/ellipsoids3d.md).
To draw a 3×3 covariance matrix, take its eigendecomposition:

- the half sizes of the ellipsoid are `k * sqrt(eigenvalues)`, where `k` is the number of standard deviations,
- the rotation is the matrix of eigenvectors (one eigenvector per column).

Put the rotation and the mean in a [`Transform3D`](../../reference/types/archetypes/transform3d.md) on the same entity:

```python
import numpy as np
import rerun as rr

rr.init("rerun_example_covariance_ellipsoid", spawn=True)

mean = np.array([1.0, 2.0, 0.5])
covariance = np.array([
    [0.40, 0.10, 0.00],
    [0.10, 0.20, 0.05],
    [0.00, 0.05, 0.10],
])

eigenvalues, eigenvectors = np.linalg.eigh(covariance)
k = 2.0  # Two standard deviations

rr.log("estimate/covariance", rr.Transform3D(translation=mean, mat3x3=eigenvectors))
rr.log("estimate/covariance", rr.Ellipsoids3D(half_sizes=[k * np.sqrt(eigenvalues)]))
```

To draw many ellipsoids in one entity, pass `centers` and one rotation per ellipsoid with `quaternions` instead.
You can convert a rotation matrix to a quaternion with, for example, `scipy.spatial.transform.Rotation.from_matrix(eigenvectors).as_quat()` (which returns `xyzw`).
If `np.linalg.det(eigenvectors)` is negative, flip the sign of one column first, so that the matrix is a rotation.

## Covariance ellipses in 2D

[`Ellipses2D`](../../reference/types/archetypes/ellipses2d.md) draws ellipses aligned with the X and Y axes.
It has no rotation, so for a 2×2 covariance with correlation, draw the outline as a [`LineStrips2D`](../../reference/types/archetypes/line_strips2d.md):

```python
mean = np.array([50.0, 40.0])
covariance = np.array([[30.0, 12.0], [12.0, 10.0]])
k = 2.0

eigenvalues, eigenvectors = np.linalg.eigh(covariance)
angles = np.linspace(0.0, 2.0 * np.pi, 64)
unit_circle = np.stack([np.cos(angles), np.sin(angles)])
outline = mean + (eigenvectors @ (k * np.sqrt(eigenvalues)[:, None] * unit_circle)).T

rr.log("estimate/covariance_2d", rr.LineStrips2D([outline]))
```

## Error bands in plots

For one-dimensional values over time, [`Measurements`](../../reference/types/archetypes/measurements.md) logs each value together with its variance.
The time series view draws a band of one standard deviation around the line.
`Measurements` is unstable: its data format may change in future releases.

snippet: archetypes/measurements_simple

## Gaussian splats

[`GaussianSplats3D`](../../reference/types/archetypes/gaussian_splats3d.md) draws 3D Gaussians with a center, per-axis scales (standard deviations), a rotation, and a color with opacity.
It is meant for 3D Gaussian Splatting (3DGS) reconstructions, but you can also use it to show many probability distributions at once.
`GaussianSplats3D` is unstable: its data format may change in future releases.

3DGS `.ply` files are loaded as `GaussianSplats3D` when you open them in the Viewer, or log them with `rr.log_file_from_path`.
