#!/usr/bin/env python3

"""Logs a `Transform3D` archetype for roundtrip checks."""

from __future__ import annotations

import argparse
from math import pi

import rerun as rr
from rerun.datatypes import RotationAxisAngle, TranslationAndMat3x3, TranslationRotationScale3D


def main() -> None:
    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_roundtrip_transform3d")

    rr.log("translation_and_mat3x3/identity", rr.Transform3D(TranslationAndMat3x3()))

    rr.log(
        "translation_and_mat3x3/translation",
        rr.Transform3D(TranslationAndMat3x3(translation=[1, 2, 3], from_parent=True)),
    )

    rr.log(
        "translation_and_mat3x3/rotation",
        rr.Transform3D(TranslationAndMat3x3(mat3x3=[1, 2, 3, 4, 5, 6, 7, 8, 9])),
    )

    rr.log(
        "translation_rotation_scale/identity",
        rr.Transform3D(TranslationRotationScale3D()),
    )

    rr.log(
        "translation_rotation_scale/translation_scale",
        rr.Transform3D(TranslationRotationScale3D(translation=[1, 2, 3], scale=42, from_parent=True)),
    )

    rr.log(
        "translation_rotation_scale/rigid",
        rr.Transform3D(
            TranslationRotationScale3D(
                translation=[1, 2, 3],
                rotation=RotationAxisAngle([0.2, 0.2, 0.8], pi),
            )
        ),
    )

    rr.log(
        "translation_rotation_scale/affine",
        rr.Transform3D(
            TranslationRotationScale3D(
                translation=[1, 2, 3],
                rotation=RotationAxisAngle([0.2, 0.2, 0.8], pi),
                scale=42,
                from_parent=True,
            )
        ),
    )

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
