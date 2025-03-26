#!/usr/bin/env python3

"""Logs a `Transform3D` archetype for roundtrip checks."""

from __future__ import annotations

import argparse
from math import pi

import rerun as rr
from rerun.datatypes import RotationAxisAngle


def main() -> None:
    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_roundtrip_transform3d")

    rr.log(
        "transform/translation",
        rr.Transform3D(translation=[1, 2, 3], relation=rr.TransformRelation.ChildFromParent),
    )

    rr.log(
        "transform/rotation",
        rr.Transform3D(mat3x3=[1, 2, 3, 4, 5, 6, 7, 8, 9]),
    )

    rr.log(
        "transform/translation_scale",
        rr.Transform3D(translation=[1, 2, 3], scale=42, relation=rr.TransformRelation.ChildFromParent),
    )

    rr.log(
        "transform/rigid",
        rr.Transform3D(
            translation=[1, 2, 3],
            rotation=RotationAxisAngle([0.2, 0.2, 0.8], pi),
        ),
    )

    rr.log(
        "transform/affine",
        rr.Transform3D(
            translation=[1, 2, 3],
            rotation=RotationAxisAngle([0.2, 0.2, 0.8], pi),
            scale=42,
            relation=rr.TransformRelation.ChildFromParent,
        ),
    )

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
