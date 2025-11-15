#!/usr/bin/env python3
"""
Demonstrates anamorphic pinhole camera support in Rerun.

This example shows the difference between symmetric cameras (fx = fy)
and anamorphic cameras (fx ≠ fy) by rendering a test pattern grid.
"""

from __future__ import annotations

import argparse

import numpy as np
import rerun as rr


def create_test_grid(width: int, height: int) -> np.ndarray:
    """Create a checkerboard test pattern to visualize camera distortion."""
    image = np.zeros((height, width, 3), dtype=np.uint8)

    # Create checkerboard pattern
    square_size = 40
    for y in range(0, height, square_size):
        for x in range(0, width, square_size):
            if ((x // square_size) + (y // square_size)) % 2 == 0:
                image[y : y + square_size, x : x + square_size] = [200, 200, 200]

    # Add grid lines
    for y in range(0, height, square_size):
        image[y : min(y + 2, height), :] = [100, 150, 255]
    for x in range(0, width, square_size):
        image[:, x : min(x + 2, width)] = [100, 150, 255]

    # Add center crosshair
    center_x, center_y = width // 2, height // 2
    image[center_y - 2 : center_y + 2, :] = [255, 0, 0]
    image[:, center_x - 2 : center_x + 2] = [255, 0, 0]

    return image


def log_camera_with_image(
    path: str,
    focal_length: float | tuple[float, float],
    width: int,
    height: int,
    image: np.ndarray,
    description: str,
) -> None:
    """Log a pinhole camera with a test image."""
    rr.log(path, rr.ViewCoordinates.RDF, static=True)

    # Log the pinhole camera
    rr.log(
        path,
        rr.Pinhole(
            focal_length=focal_length,
            width=width,
            height=height,
        ),
    )

    # Log the test image
    rr.log(path, rr.Image(image))

    # Log a text annotation describing the camera
    rr.log(f"{path}/description", rr.TextDocument(description, media_type=rr.MediaType.MARKDOWN))


def log_3d_reference_points() -> None:
    """Log 3D points that will be projected by the cameras."""
    # Create a 3D grid of points in front of the camera
    points = []
    colors = []

    # Grid in 3D space
    for x in np.linspace(-2, 2, 9):
        for y in np.linspace(-1.5, 1.5, 7):
            z = 5.0  # Distance from camera
            points.append([x, y, z])

            # Color based on position
            r = int(255 * (x + 2) / 4)
            g = int(255 * (y + 1.5) / 3)
            b = 150
            colors.append([r, g, b])

    rr.log(
        "world/reference_points",
        rr.Points3D(
            positions=points,
            colors=colors,
            radii=0.05,
        ),
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--camera-type",
        choices=["all", "symmetric", "anamorphic"],
        default="all",
        help="Which camera type to demonstrate",
    )
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_anamorphic_camera")

    # Image dimensions
    width, height = 640, 480

    # Create test pattern
    test_image = create_test_grid(width, height)

    # Log 3D reference points
    log_3d_reference_points()

    if args.camera_type in ["all", "symmetric"]:
        # Symmetric camera (standard pinhole, fx = fy)
        fx = fy = 500.0
        log_camera_with_image(
            "world/camera_symmetric",
            focal_length=fx,
            width=width,
            height=height,
            image=test_image,
            description=f"""
# Symmetric Camera

Standard pinhole camera with equal focal lengths:
- fx = {fx:.1f} pixels
- fy = {fy:.1f} pixels
- fx = fy (symmetric/isotropic)

This is the typical camera model.
            """,
        )

    if args.camera_type in ["all", "anamorphic"]:
        # Anamorphic camera (fx ≠ fy)
        fx, fy = 700.0, 400.0  # Significant difference
        log_camera_with_image(
            "world/camera_anamorphic",
            focal_length=[fx, fy],
            width=width,
            height=height,
            image=test_image,
            description=f"""
# Anamorphic Camera

Anamorphic pinhole camera with different focal lengths:
- fx = {fx:.1f} pixels
- fy = {fy:.1f} pixels
- fx/fy ratio = {fx/fy:.2f}

This camera model is used for:
- Non-square pixels
- Anamorphic lenses
- Some industrial/scientific cameras
            """,
        )

        # More extreme example
        fx2, fy2 = 800.0, 300.0
        log_camera_with_image(
            "world/camera_extreme_anamorphic",
            focal_length=[fx2, fy2],
            width=width,
            height=height,
            image=test_image,
            description=f"""
# Extreme Anamorphic Camera

Very anamorphic pinhole camera:
- fx = {fx2:.1f} pixels
- fy = {fy2:.1f} pixels
- fx/fy ratio = {fx2/fy2:.2f}

This demonstrates the handling of extreme cases.
            """,
        )

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
