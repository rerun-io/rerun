#!/usr/bin/env python3
"""Test to verify anamorphic camera support."""

from __future__ import annotations

import numpy as np
import rerun as rr


def test_symmetric_camera() -> None:
    """Test that symmetric cameras (fx = fy) can be created and logged."""
    rr.init("test_symmetric_camera", spawn=False)
    rr.memory_recording()

    width, height = 640, 480
    image = np.zeros((height, width, 3), dtype=np.uint8)

    # Log symmetric camera with scalar focal length
    rr.log("camera_symmetric", rr.ViewCoordinates.RDF, static=True)
    rr.log("camera_symmetric", rr.Pinhole(focal_length=500.0, width=width, height=height))
    rr.log("camera_symmetric", rr.Image(image))

    # If we get here without exceptions, the test passes
    assert True


def test_anamorphic_camera() -> None:
    """Test that anamorphic cameras (fx ≠ fy) can be created and logged."""
    rr.init("test_anamorphic_camera", spawn=False)
    rr.memory_recording()

    width, height = 640, 480
    image = np.zeros((height, width, 3), dtype=np.uint8)

    # Log anamorphic camera with tuple focal length
    rr.log("camera_anamorphic", rr.ViewCoordinates.RDF, static=True)
    rr.log("camera_anamorphic", rr.Pinhole(focal_length=[700.0, 400.0], width=width, height=height))
    rr.log("camera_anamorphic", rr.Image(image))

    # If we get here without exceptions, the test passes
    assert True


def test_anamorphic_with_3d_points() -> None:
    """Test that anamorphic cameras work with 3D point clouds."""
    rr.init("test_anamorphic_with_3d_points", spawn=False)
    rr.memory_recording()

    width, height = 640, 480

    # Create test cameras
    rr.log("world/camera_sym", rr.ViewCoordinates.RDF, static=True)
    rr.log("world/camera_sym", rr.Pinhole(focal_length=500.0, width=width, height=height))

    rr.log("world/camera_anam", rr.ViewCoordinates.RDF, static=True)
    rr.log("world/camera_anam", rr.Pinhole(focal_length=[700.0, 400.0], width=width, height=height))

    # Log 3D reference points
    points = []
    for x in np.linspace(-1, 1, 5):
        for y in np.linspace(-1, 1, 5):
            points.append([x, y, 3.0])

    rr.log("world/points", rr.Points3D(positions=points, radii=0.05))

    # If we get here without exceptions, the test passes
    assert True
