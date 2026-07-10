"""Log several 3D geometry primitives."""

import numpy as np

import rerun as rr

rr.init("rerun_example_geometry3d_primitives", spawn=True)

texture = np.zeros((96, 96, 4), dtype=np.uint8)
texture[:, :, 3] = 255
texture[:48, :48, :3] = [0, 216, 255]
texture[:48, 48:, :3] = [255, 210, 0]
texture[48:, :48, :3] = [255, 84, 170]
texture[48:, 48:, :3] = [255, 255, 255]

rr.log(
    "cones",
    rr.Cones3D(
        lengths=[1.6, 2.2, 1.2],
        radii=[0.6, 0.35, 0.8],
        centers=[(-2.0, 0.0, 0.0), (-0.5, 0.0, 0.3), (1.0, 0.0, -0.2)],
        albedo_texture=texture,
    ),
)

rr.log(
    "rays",
    rr.Rays3D(
        origins=[(-2.7, -1.5, 0.0), (-1.2, -1.5, 0.0), (0.3, -1.5, 0.0)],
        vectors=[(0.8, 0.7, 0.4), (0.4, 1.0, 0.8), (1.0, 0.4, 0.2)],
        radii=[0.025],
        colors=[(80, 220, 180)],
    ),
)

rr.log(
    "planes",
    rr.Planes3D(
        planes=[
            rr.components.Plane3D.XY.with_distance(-0.75),
            rr.components.Plane3D([0.5, 0.0, 1.0], 0.2),
        ],
        half_sizes=[(2.5, 1.2), (1.4, 1.0)],
        albedo_texture=texture,
        albedo_factor=(255, 255, 255, 160),
        fill_mode=rr.components.FillMode.TransparentFillMajorWireframe,
    ),
)

rr.log(
    "triangles",
    rr.Triangles3D(
        vertex_positions=[
            (-2.0, 1.4, 0.0),
            (-1.0, 1.4, 0.0),
            (-1.5, 2.2, 0.6),
            (0.0, 1.4, 0.0),
            (1.0, 1.4, 0.0),
            (0.5, 2.2, 0.6),
        ],
        vertex_texcoords=[(0, 0), (1, 0), (0.5, 1), (0, 0), (1, 0), (0.5, 1)],
        albedo_texture=texture,
        fill_mode=rr.components.FillMode.TransparentFillMajorWireframe,
    ),
)
