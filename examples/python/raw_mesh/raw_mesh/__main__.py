#!/usr/bin/env python3
"""Shows how to log procedurally generated raw 3D meshes and a transform hierarchy."""

from __future__ import annotations

import argparse
import math
from collections.abc import Sequence

import rerun as rr  # pip install rerun-sdk
import rerun.blueprint as rrb

DESCRIPTION = """
# Raw meshes
This example builds a small scene directly from vertices and triangles, then logs it as [`Mesh3D`][mesh].
The scene also demonstrates how child meshes inherit their parents' [`Transform3D`][transform].

[mesh]: https://rerun.io/docs/reference/types/archetypes/mesh3d
[transform]: https://rerun.io/docs/reference/types/archetypes/transform3d
"""

Vec3 = tuple[float, float, float]
Triangle = tuple[int, int, int]
Rgb = tuple[int, int, int]


def _triangle_soup(
    vertices: Sequence[Vec3],
    triangles: Sequence[Triangle],
    face_colors: Sequence[Rgb],
) -> tuple[list[Vec3], list[Vec3], list[Rgb]]:
    """Expand indexed triangles and compute one normal and color per face."""
    positions: list[Vec3] = []
    normals: list[Vec3] = []
    colors: list[Rgb] = []

    for (i0, i1, i2), color in zip(triangles, face_colors, strict=True):
        p0, p1, p2 = vertices[i0], vertices[i1], vertices[i2]
        edge1 = tuple(b - a for a, b in zip(p0, p1, strict=True))
        edge2 = tuple(b - a for a, b in zip(p0, p2, strict=True))
        normal = (
            edge1[1] * edge2[2] - edge1[2] * edge2[1],
            edge1[2] * edge2[0] - edge1[0] * edge2[2],
            edge1[0] * edge2[1] - edge1[1] * edge2[0],
        )
        length = math.sqrt(sum(component * component for component in normal))
        unit_normal = tuple(component / length for component in normal)

        positions.extend((p0, p1, p2))
        normals.extend((unit_normal, unit_normal, unit_normal))
        colors.extend((color, color, color))

    return positions, normals, colors


def _box(size: Vec3) -> rr.Mesh3D:
    """Create a box as a non-indexed triangle soup with per-face colors."""
    x, y, z = (dimension / 2.0 for dimension in size)
    vertices = [
        (-x, -y, -z),
        (x, -y, -z),
        (x, y, -z),
        (-x, y, -z),
        (-x, -y, z),
        (x, -y, z),
        (x, y, z),
        (-x, y, z),
    ]
    triangles = [
        (0, 2, 1),
        (0, 3, 2),
        (4, 5, 6),
        (4, 6, 7),
        (0, 1, 5),
        (0, 5, 4),
        (1, 2, 6),
        (1, 6, 5),
        (2, 3, 7),
        (2, 7, 6),
        (3, 0, 4),
        (3, 4, 7),
    ]
    face_colors = [
        (95, 145, 255),
        (95, 145, 255),
        (80, 120, 220),
        (80, 120, 220),
        (120, 165, 255),
        (120, 165, 255),
        (65, 105, 205),
        (65, 105, 205),
        (110, 155, 245),
        (110, 155, 245),
        (75, 115, 215),
        (75, 115, 215),
    ]
    positions, normals, colors = _triangle_soup(vertices, triangles, face_colors)
    return rr.Mesh3D(vertex_positions=positions, vertex_normals=normals, vertex_colors=colors)


def _pyramid() -> rr.Mesh3D:
    """Create an indexed pyramid to demonstrate triangle indices and a material color."""
    return rr.Mesh3D(
        vertex_positions=[
            (-0.45, -0.45, 0.0),
            (0.45, -0.45, 0.0),
            (0.45, 0.45, 0.0),
            (-0.45, 0.45, 0.0),
            (0.0, 0.0, 0.8),
        ],
        triangle_indices=[
            (0, 2, 1),
            (0, 3, 2),
            (0, 1, 4),
            (1, 2, 4),
            (2, 3, 4),
            (3, 0, 4),
        ],
        albedo_factor=(255, 170, 60),
    )


def log_scene() -> None:
    """Log a small hierarchy assembled from raw mesh data."""
    rr.log("world", rr.ViewCoordinates.RFU, static=True)

    rr.log("world/base", _box((2.6, 1.8, 0.35)), static=True)

    rr.log("world/base/arm", rr.Transform3D(translation=(0.0, 0.0, 0.9)), static=True)
    rr.log("world/base/arm", _box((0.45, 0.45, 1.5)), static=True)

    rr.log("world/base/arm/tool", rr.Transform3D(translation=(0.0, 0.0, 1.1)), static=True)
    rr.log("world/base/arm/tool", _pyramid(), static=True)


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Logs procedurally generated raw 3D meshes and their transform hierarchy.",
    )
    rr.script_add_args(parser)
    args = parser.parse_args()

    blueprint = rrb.Horizontal(
        rrb.Spatial3DView(name="Mesh", origin="/world"),
        rrb.TextDocumentView(name="Description", origin="/description"),
        column_shares=[3, 1],
    )

    rr.script_setup(args, "rerun_example_raw_mesh", default_blueprint=blueprint)
    rr.log("description", rr.TextDocument(DESCRIPTION, media_type=rr.MediaType.MARKDOWN), static=True)
    log_scene()
    rr.script_teardown(args)


if __name__ == "__main__":
    main()
