"""Draw a 2D covariance matrix as a rotated ellipse outline."""

import numpy as np

import rerun as rr

rr.init("rerun_example_spatial2d_covariance", spawn=True)

mean = np.array([50.0, 40.0])
covariance = np.array([[30.0, 12.0], [12.0, 10.0]])
k = 2.0

eigenvalues, eigenvectors = np.linalg.eigh(covariance)
angles = np.linspace(0.0, 2.0 * np.pi, 64)
unit_circle = np.stack([np.cos(angles), np.sin(angles)])
half_axes = k * np.sqrt(eigenvalues)[:, None]
outline = mean + (eigenvectors @ (half_axes * unit_circle)).T

rr.log("estimate/covariance_2d", rr.LineStrips2D([outline]))
