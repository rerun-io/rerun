"""Draw a 3D covariance matrix as an ellipsoid."""

import numpy as np

import rerun as rr

rr.init("rerun_example_spatial3d_covariance", spawn=True)

mean = np.array([1.0, 2.0, 0.5])
covariance = np.array([
    [0.40, 0.10, 0.00],
    [0.10, 0.20, 0.05],
    [0.00, 0.05, 0.10],
])

eigenvalues, eigenvectors = np.linalg.eigh(covariance)
k = 2.0  # Two standard deviations

rr.log(
    "estimate/covariance",
    rr.Transform3D(translation=mean, mat3x3=eigenvectors),
)
rr.log(
    "estimate/covariance",
    rr.Ellipsoids3D(half_sizes=[k * np.sqrt(eigenvalues)]),
)
