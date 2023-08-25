#!/usr/bin/env python3

"""Logs a `Transform3D` archetype for roundtrip checks."""

from __future__ import annotations

import argparse
from math import pi

import rerun as rr
import rerun.experimental as rr2
from rerun.experimental import dt as rrd


def main() -> None:
    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun-example-roundtrip_transform3d")

    rr2.log("translation_and_mat3x3/identity", rr2.Transform3D(rrd.TranslationAndMat3x3()))

    rr2.log(
        "translation_and_mat3x3/translation",
        rr2.Transform3D(rrd.TranslationAndMat3x3(translation=[1, 2, 3], from_parent=True)),
    )

    rr2.log(
        "translation_and_mat3x3/rotation",
        rr2.Transform3D(rrd.TranslationAndMat3x3(matrix=[1, 2, 3, 4, 5, 6, 7, 8, 9])),
    )

    rr2.log(
        "translation_rotation_scale/identity",
        rr2.Transform3D(rrd.TranslationRotationScale3D()),
    )

    rr2.log(
        "translation_rotation_scale/translation_scale",
        rr2.Transform3D(rrd.TranslationRotationScale3D(translation=[1, 2, 3], scale=42, from_parent=True)),
    )

    rr2.log(
        "translation_rotation_scale/rigid",
        rr2.Transform3D(
            rrd.TranslationRotationScale3D(
                translation=[1, 2, 3],
                rotation=rrd.RotationAxisAngle([0.2, 0.2, 0.8], pi),
            )
        ),
    )

    rr2.log(
        "translation_rotation_scale/affine",
        rr2.Transform3D(
            rrd.TranslationRotationScale3D(
                translation=[1, 2, 3],
                rotation=rrd.RotationAxisAngle([0.2, 0.2, 0.8], pi),
                scale=42,
                from_parent=True,
            )
        ),
    )

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
